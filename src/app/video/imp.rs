use flume::{Receiver, Sender};
use gdk_wayland::{WaylandDisplay, wayland_client::Proxy};
use gtk::{
    gdk::GLContext,
    glib::{self, Propagation, Properties, Variant, clone, subclass::Signal},
    prelude::*,
    subclass::prelude::*,
};
use libmpv2::{
    Error, Format, Mpv, SetData,
    events::{Event, PropertyData},
    mpv_end_file_reason,
    render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType},
};
use std::{cell::RefCell, env, os::raw::c_void, sync::OnceLock};
use tracing::error;

use crate::spawn_local;

fn get_proc_address(_context: &GLContext, name: &str) -> *mut c_void {
    epoxy::get_proc_addr(name) as _
}

enum EventCallback {
    Render,
    Events,
}

#[derive(Properties)]
#[properties(wrapper_type = super::Video)]
pub struct Video {
    // A RenderContext borrows the Mpv it was created from, and a GObject has
    // no lifetime parameter to tie the two together, so the handle is leaked
    // to give it a 'static one. One Video exists per application run, and
    // realize/unrealize only replaces the render context, so this is a single
    // allocation rather than a growing leak.
    mpv: &'static Mpv,
    events: Sender<EventCallback>,
    event_receiver: RefCell<Option<Receiver<EventCallback>>>,
    render_context: RefCell<Option<RenderContext<'static>>>,
}

impl Default for Video {
    fn default() -> Self {
        let log = env::var("RUST_LOG");
        let msg_level = match log {
            Ok(scope) => &format!("all={}", scope.as_str()),
            _ => "all=no",
        };

        // Required for libmpv to work alongside GTK
        gettextrs::setlocale(gettextrs::LocaleCategory::LcNumeric, "C")
            .expect("Failed to set LC_NUMERIC to C");

        let mut mpv = Mpv::with_initializer(|init| {
            init.set_property("vo", "libmpv")?;
            init.set_property("video-timing-offset", "0")?;
            init.set_property("video-sync", "audio")?;
            init.set_property("terminal", "yes")?;
            init.set_property("msg-level", msg_level)?;
            Ok(())
        })
        .expect("Failed to create mpv");

        mpv.disable_deprecated_events().ok();

        // Set up while the handle is still owned here: the wakeup callback is
        // the only thing that needs it mutably, and it does not depend on the
        // widget being realized.
        let (sender, receiver) = flume::unbounded::<EventCallback>();
        let wakeup_sender = sender.clone();
        mpv.set_wakeup_callback(move || {
            wakeup_sender.send(EventCallback::Events).ok();
        });

        Self {
            mpv: Box::leak(Box::new(mpv)),
            events: sender,
            event_receiver: RefCell::new(Some(receiver)),
            render_context: Default::default(),
        }
    }
}

impl Video {
    fn on_event<T: Fn(Event)>(&self, callback: T) {
        while let Some(result) = self.mpv.wait_event(0.0) {
            match result {
                Ok(event) => callback(event),
                Err(Error::Raw(e)) => {
                    error!("MPV errored with: {e}");
                    self.obj()
                        .emit_by_name::<()>("playback-ended", &[&"error".to_string()]);
                }
                Err(e) => error!("Failed to wait for event: {e}"),
            }
        }
    }

    fn unobserve_properties(&self) {
        if let Err(e) = self.mpv.unobserve_property(0) {
            error!("Failed to unobserve properties: {e}");
        }
    }

    pub fn send_command(&self, name: &str, args: &[&str]) {
        if let Err(e) = self.mpv.command(name, args) {
            error!("Failed to send command {name}: {e}");
        }
    }

    pub fn observe_property(&self, name: &str, format: Format) {
        if let Err(e) = self.mpv.observe_property(name, format, 0) {
            error!("Failed to observe property {name}: {e}");
        }
    }

    pub fn set_property<T: SetData>(&self, name: &str, value: T) {
        if let Err(e) = self.mpv.set_property(name, value) {
            error!("Failed to set property {name}: {e}");
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Video {
    const NAME: &'static str = "Video";
    type Type = super::Video;
    type ParentType = gtk::GLArea;
}

#[glib::derived_properties]
impl ObjectImpl for Video {
    fn signals() -> &'static [Signal] {
        static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
        SIGNALS.get_or_init(|| {
            vec![
                Signal::builder("property-changed")
                    .param_types([str::static_type(), Variant::static_type()])
                    .build(),
                Signal::builder("playback-ended")
                    .param_types([str::static_type()])
                    .build(),
            ]
        })
    }
}

impl WidgetImpl for Video {
    fn realize(&self) {
        self.parent_realize();

        let object = self.obj();
        object.make_current();

        if object.error().is_some() {
            return;
        }

        if let Some(context) = object.context() {
            // Only on the first realize: the channel outlives unrealize, so a
            // second realize reuses the task already draining it.
            if let Some(receiver) = self.event_receiver.borrow_mut().take() {
                spawn_local!(clone!(
                    #[weak(rename_to = video)]
                    self,
                    #[weak]
                    object,
                    async move {
                        while let Ok(event) = receiver.recv_async().await {
                            match event {
                                EventCallback::Render => {
                                    object.queue_render();
                                }
                                EventCallback::Events => {
                                    video.on_event(|event| match event {
                                        Event::PropertyChange { name, change, .. } => {
                                            let value = match change {
                                                PropertyData::Str(v) => Some(v.to_variant()),
                                                PropertyData::Flag(v) => Some(v.to_variant()),
                                                PropertyData::Double(v) => Some(v.to_variant()),
                                                _ => None,
                                            };

                                            if let Some(value) = value {
                                                object.emit_by_name::<()>(
                                                    "property-changed",
                                                    &[&name, &value],
                                                );
                                            }
                                        }
                                        Event::EndFile(reason) => {
                                            let reason = match reason {
                                                mpv_end_file_reason::Eof => "eof".to_string(),
                                                mpv_end_file_reason::Stop => "stop".to_string(),
                                                mpv_end_file_reason::Redirect => {
                                                    "redirect".to_string()
                                                }
                                                mpv_end_file_reason::Error => "error".to_string(),
                                                mpv_end_file_reason::Quit => "quit".to_string(),
                                                _ => "other".to_string(),
                                            };

                                            object.emit_by_name::<()>("playback-ended", &[&reason]);
                                            video.unobserve_properties();
                                        }
                                        _ => {}
                                    });
                                }
                            }
                        }
                    }
                ));
            }

            let mut render_params = vec![
                RenderParam::ApiType(RenderParamApiType::OpenGl),
                RenderParam::InitParams(OpenGLInitParams {
                    get_proc_address,
                    ctx: context,
                }),
            ];

            let display = object.display();
            if let Ok(display) = display.downcast::<WaylandDisplay>()
                && let Some(display) = display.wl_display()
            {
                render_params.push(RenderParam::WaylandDisplay(
                    display.id().as_ptr() as *const c_void
                ));
            }

            let mut render_context = self
                .mpv
                .create_render_context(render_params)
                .expect("Failed to create render context");

            let render_sender = self.events.clone();
            render_context.set_update_callback(move || {
                render_sender.send(EventCallback::Render).ok();
            });

            *self.render_context.borrow_mut() = Some(render_context);
        }
    }

    fn unrealize(&self) {
        self.obj().make_current();
        if let Some(render_context) = self.render_context.borrow_mut().take() {
            drop(render_context);
        }

        self.parent_unrealize();
    }
}

impl GLAreaImpl for Video {
    fn render(&self, _context: &GLContext) -> Propagation {
        let object = self.obj();

        let mut fbo = 0;
        unsafe {
            epoxy::GetIntegerv(epoxy::FRAMEBUFFER_BINDING, &mut fbo);
        }

        let scale_factor = object.scale_factor();
        let width = object.width() * scale_factor;
        let height = object.height() * scale_factor;

        if let Some(ref render_context) = *self.render_context.borrow() {
            render_context
                .render::<GLContext>(fbo, width, height, true)
                .expect("Failed to render");
        }

        Propagation::Stop
    }
}
