use gdk_wayland::{WaylandDisplay, wayland_client::Proxy};
use gtk::{
    gdk::GLContext,
    glib::{self, ControlFlow, Propagation, Properties, Variant, clone, subclass::Signal},
    prelude::*,
    subclass::prelude::*,
};
use libmpv2::{
    Format, Mpv, SetData,
    events::{Event, PropertyData},
    mpv_end_file_reason,
    render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType},
};
use std::{cell::RefCell, env, os::raw::c_void, sync::OnceLock};
use tracing::error;

use crate::spawn_local;

/// How often mpv's event queue is drained (~60 Hz). Small enough that playback
/// state and observed properties reach the UI promptly, large enough that the
/// GLib main loop still sleeps between ticks so idle CPU stays negligible.
const EVENT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(16);

fn get_proc_address(_context: &GLContext, name: &str) -> *mut c_void {
    epoxy::get_proc_addr(name) as _
}

#[derive(Properties)]
#[properties(wrapper_type = super::Video)]
pub struct Video {
    mpv: RefCell<Mpv>,
    render_context: RefCell<Option<RenderContext>>,
}

impl Default for Video {
    fn default() -> Self {
        let log = env::var("RUST_LOG");
        let msg_level = match log {
            Ok(scope) => &format!("all={}", scope.as_str()),
            _ => "all=no",
        };

        let mpv = Mpv::with_initializer(|init| {
            init.set_property("vo", "libmpv")?;
            init.set_property("video-timing-offset", "0")?;
            init.set_property("video-sync", "audio")?;
            // Enable zero-copy hardware decoding by default. mpv otherwise
            // defaults to software decoding, and the web UI only ever asks for
            // the copy-back path (`hwdec=auto-copy`, remapped in mod.rs). Our GL
            // render context is created with the Wayland display, so mpv can use
            // the EGL/dmabuf interop (VAAPI on Mesa) and keep frames on the GPU.
            // `auto-safe` falls back to software when no safe interop exists.
            init.set_property("hwdec", "auto-safe")?;
            init.set_property("terminal", "yes")?;
            init.set_property("msg-level", msg_level)?;
            Ok(())
        })
        .expect("Failed to create mpv");

        mpv.disable_deprecated_events().ok();

        Self {
            mpv: RefCell::new(mpv),
            render_context: Default::default(),
        }
    }
}

impl Video {
    fn process_events<T: Fn(Event)>(&self, callback: T) {
        loop {
            match self.mpv.borrow_mut().wait_event(0.0) {
                Some(Ok(event)) => callback(event),
                Some(Err(e)) => {
                    error!("Failed to wait for event: {e}");
                    break;
                }
                None => break,
            }
        }
    }

    pub fn send_command(&self, name: &str, args: &[&str]) {
        if let Err(e) = self.mpv.borrow().command(name, args) {
            error!("Failed to send command {name}: {e}");
        }
    }

    pub fn observe_property(&self, name: &str, format: Format) {
        if let Err(e) = self.mpv.borrow().observe_property(name, format, 0) {
            error!("Failed to observe property {name}: {e}");
        }
    }

    pub fn set_property<T: SetData>(&self, name: &str, value: T) {
        if let Err(e) = self.mpv.borrow().set_property(name, value) {
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
                Signal::builder("playback-started").build(),
                Signal::builder("playback-ended")
                    .param_types([str::static_type()])
                    .build(),
            ]
        })
    }

    fn constructed(&self) {
        self.parent_constructed();

        // Drain mpv's event queue on a timer.
        //
        // We deliberately do NOT drive this purely from `mpv_set_wakeup_callback`.
        // That callback is only a best-effort "there might be new events" hint:
        // mpv coalesces property changes (they are only produced once the queue
        // drains to MPV_EVENT_NONE) and explicitly "there's only one wakeup
        // callback invocation for multiple events", plus there is an inherent
        // lost-wakeup race between the callback firing and `wait_event` clearing
        // the pending-wakeup flag. Relying on it alone latches after the first
        // burst, so StartFile / property changes / EndFile never reach the web UI
        // and playback appears stuck (the Stremio player sits on the loading
        // screen showing 0 peers).
        //
        // A `timeout` source drains the whole queue unconditionally every tick,
        // so no event is lost. Unlike the previous `idle_add_local`, a timeout
        // lets the GLib main loop sleep between ticks, keeping idle CPU low.
        glib::timeout_add_local(
            EVENT_POLL_INTERVAL,
            clone!(
                #[weak(rename_to = video)]
                self,
                #[weak(rename_to = object)]
                self.obj(),
                #[upgrade_or]
                ControlFlow::Break,
                move || {
                    video.process_events(|event| match event {
                        Event::PropertyChange { name, change, .. } => {
                            let value = match change {
                                PropertyData::Str(v) => Some(v.to_variant()),
                                PropertyData::Flag(v) => Some(v.to_variant()),
                                PropertyData::Double(v) => Some(v.to_variant()),
                                _ => None,
                            };

                            if let Some(value) = value {
                                object.emit_by_name::<()>("property-changed", &[&name, &value]);
                            }
                        }
                        Event::StartFile => {
                            object.emit_by_name::<()>("playback-started", &[]);
                        }
                        Event::EndFile(reason) => {
                            let reason = match reason {
                                mpv_end_file_reason::Eof => "eof".to_string(),
                                mpv_end_file_reason::Stop => "stop".to_string(),
                                mpv_end_file_reason::Redirect => "redirect".to_string(),
                                mpv_end_file_reason::Error => "error".to_string(),
                                mpv_end_file_reason::Quit => "quit".to_string(),
                                _ => "other".to_string(),
                            };

                            object.emit_by_name::<()>("playback-ended", &[&reason]);
                        }
                        _ => {}
                    });

                    ControlFlow::Continue
                }
            ),
        );
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
            let mut mpv = self.mpv.borrow_mut();
            let mpv_handle = unsafe { mpv.ctx.as_mut() };

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

            let mut render_context = RenderContext::new(mpv_handle, render_params)
                .expect("Failed to create render context");

            let (sender, receiver) = flume::unbounded::<()>();

            spawn_local!(clone!(
                #[weak]
                object,
                async move {
                    while receiver.recv_async().await.is_ok() {
                        // Drain any additional pending updates so a burst of
                        // update callbacks only triggers a single redraw.
                        while receiver.try_recv().is_ok() {}

                        object.queue_render();
                    }
                }
            ));

            render_context.set_update_callback(move || {
                sender.send(()).ok();
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
