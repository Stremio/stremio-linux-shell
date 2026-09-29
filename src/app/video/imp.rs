use gdk_wayland::{WaylandDisplay, wayland_client::Proxy};
use gtk::{
    gdk::GLContext,
    glib::{self, Propagation, Properties, Variant, clone, subclass::Signal},
    prelude::*,
    subclass::prelude::*,
};
use libmpv2::{
    Format, Mpv, SetData,
    events::{Event, PropertyData},
    mpv_end_file_reason,
    render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType},
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    env,
    os::raw::c_void,
    sync::OnceLock,
};
use tracing::error;

use crate::spawn_local;

fn get_proc_address(_context: &GLContext, name: &str) -> *mut c_void {
    epoxy::get_proc_addr(name) as _
}

enum EventCallback {
    Render,
    Events,
}

type Submit = Box<dyn FnOnce(&Mpv, u64) -> libmpv2::Result<()>>;

struct Control {
    name: String,
    submit: Submit,
}

impl Control {
    fn is_subtitle(&self) -> bool {
        matches!(self.name.as_str(), "sub-add" | "sub-remove" | "sid")
    }
}

#[derive(Properties)]
#[properties(wrapper_type = super::Video)]
pub struct Video {
    mpv: RefCell<Mpv>,
    render_context: RefCell<Option<RenderContext>>,
    controls: RefCell<VecDeque<Control>>,
    pending_control: RefCell<Option<(u64, String)>>,
    pending_subtitle: Cell<Option<u64>>,
    next_request: Cell<u64>,
}

impl Default for Video {
    fn default() -> Self {
        let msg_level = match env::var("RUST_LOG").as_deref() {
            Ok("error") => "all=error",
            Ok("warn") => "all=warn",
            Ok("info") => "all=info",
            Ok("debug") => "all=debug",
            Ok("trace") => "all=trace",
            _ => "all=no",
        };

        // Required for libmpv to work alongside GTK
        gettextrs::setlocale(gettextrs::LocaleCategory::LcNumeric, "C")
            .expect("Failed to set LC_NUMERIC to C");

        let mpv = Mpv::with_initializer(|init| {
            init.set_property("vo", "libmpv")?;
            init.set_property("video-timing-offset", "0")?;
            init.set_property("video-sync", "audio")?;
            init.set_property("terminal", "yes")?;
            init.set_property("msg-level", msg_level)?;
            Ok(())
        })
        .expect("Failed to create mpv");

        mpv.disable_deprecated_events().ok();

        Self {
            mpv: RefCell::new(mpv),
            render_context: Default::default(),
            controls: Default::default(),
            pending_control: Default::default(),
            pending_subtitle: Default::default(),
            next_request: Default::default(),
        }
    }
}

impl Video {
    fn on_event<T: Fn(Event)>(&self, callback: T) {
        while let Some(result) = self.mpv.borrow().wait_event(0.0) {
            match result {
                Ok(event) => callback(event),
                Err(e) => error!("Failed to wait for event: {e}"),
            }
        }
    }

    fn unobserve_properties(&self) {
        if let Err(e) = self.mpv.borrow().unobserve_property(0) {
            error!("Failed to unobserve properties: {e}");
        }
    }

    fn dispatch(&self) {
        while self.pending_control.borrow().is_none() {
            let control = {
                let mut controls = self.controls.borrow_mut();
                let subtitle_pending = self.pending_subtitle.get().is_some();
                controls
                    .iter()
                    .position(|control| !subtitle_pending || !control.is_subtitle())
                    .and_then(|index| controls.remove(index))
            };
            let Some(control) = control else {
                break;
            };

            let id = self.next_request.get();
            self.next_request.set(id + 1);

            match (control.submit)(&self.mpv.borrow(), id) {
                Ok(()) if control.name == "sub-add" => self.pending_subtitle.set(Some(id)),
                Ok(()) => *self.pending_control.borrow_mut() = Some((id, control.name)),
                Err(e) => error!("Failed to send {}: {e}", control.name),
            }
        }
    }

    fn enqueue(&self, control: Control) {
        self.controls.borrow_mut().push_back(control);
        self.dispatch();
    }

    fn on_reply(&self, id: u64, result: libmpv2::Result<()>) {
        let name = if self.pending_subtitle.get() == Some(id) {
            self.pending_subtitle.set(None);
            Some("sub-add".to_owned())
        } else {
            self.pending_control
                .borrow_mut()
                .take_if(|(pending, _)| *pending == id)
                .map(|(_, name)| name)
        };

        if let (Some(name), Err(e)) = (name, result) {
            error!("Failed to send {name}: {e}");
        }

        self.dispatch();
    }

    pub fn send_command(&self, name: String, args: Vec<String>) {
        if matches!(name.as_str(), "loadfile" | "stop") {
            self.controls
                .borrow_mut()
                .retain(|control| !control.is_subtitle());
            if let Some(id) = self.pending_subtitle.get() {
                self.mpv.borrow().abort_async_command(id);
            }
        }

        let command = name.clone();
        self.enqueue(Control {
            name,
            submit: Box::new(move |mpv, id| {
                let args: Vec<_> = args.iter().map(String::as_str).collect();
                mpv.command_async(&command, &args, id)
            }),
        });
    }

    pub fn observe_property(&self, name: &str, format: Format) {
        if let Err(e) = self.mpv.borrow().observe_property(name, format, 0) {
            error!("Failed to observe property {name}: {e}");
        }
    }

    pub fn set_property<T: SetData + 'static>(&self, name: &str, value: T) {
        let property = name.to_owned();
        self.enqueue(Control {
            name: name.to_owned(),
            submit: Box::new(move |mpv, id| mpv.set_property_async(&property, value, id)),
        });
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
            let mut mpv = self.mpv.borrow_mut();
            let (sender, receiver) = flume::unbounded::<EventCallback>();

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
                                            mpv_end_file_reason::Redirect => "redirect".to_string(),
                                            mpv_end_file_reason::Error => "error".to_string(),
                                            mpv_end_file_reason::Quit => "quit".to_string(),
                                            _ => "other".to_string(),
                                        };

                                        object.emit_by_name::<()>("playback-ended", &[&reason]);
                                        video.unobserve_properties();
                                    }
                                    Event::CommandReply {
                                        reply_userdata,
                                        result,
                                    }
                                    | Event::SetPropertyReply {
                                        reply_userdata,
                                        result,
                                    } => video.on_reply(reply_userdata, result),
                                    _ => {}
                                });
                            }
                        }
                    }
                }
            ));

            let wakeup_sender = sender.clone();
            mpv.set_wakeup_callback(move || {
                wakeup_sender.send(EventCallback::Events).ok();
            });

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

            let mpv_handle = unsafe { mpv.ctx.as_mut() };
            let mut render_context = RenderContext::new(mpv_handle, render_params)
                .expect("Failed to create render context");

            let render_sender = sender.clone();
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
