use gtk::{
    gdk::GLContext,
    glib::{self, Propagation, Variant, subclass::Signal},
    prelude::*,
    subclass::prelude::*,
};
use libc::{LC_NUMERIC, setlocale};
use libmpv2::{
    Format, Mpv, SetData,
    events::{Event, PropertyData},
    render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType},
};
use std::{
    cell::{Cell, RefCell},
    env,
    os::raw::c_void,
    sync::{OnceLock, mpsc::channel},
};
use tracing::error;

fn get_proc_address(_context: &GLContext, name: &str) -> *mut c_void {
    epoxy::get_proc_addr(name) as _
}

pub struct Video {
    mpv: RefCell<Mpv>,
    render_context: RefCell<Option<RenderContext>>,
    fbo: Cell<u32>,
}

impl Default for Video {
    fn default() -> Self {
        // Required for libmpv to work alongside gtk
        unsafe {
            setlocale(LC_NUMERIC, c"C".as_ptr());
        }

        let log = env::var("RUST_LOG");
        let msg_level = match log {
            Ok(scope) => &format!("all={}", scope.as_str()),
            _ => "all=no",
        };

        let mpv = Mpv::with_initializer(|init| {
            init.set_property("vo", "libmpv")?;
            init.set_property("video-timing-offset", "0")?;
            init.set_property("terminal", "yes")?;
            init.set_property("msg-level", msg_level)?;
            Ok(())
        })
        .expect("Failed to create mpv");

        mpv.disable_deprecated_events().ok();

        Self {
            mpv: RefCell::new(mpv),
            render_context: Default::default(),
            fbo: Default::default(),
        }
    }
}

impl Video {
    fn fbo(&self) -> i32 {
        let mut fbo = self.fbo.get();

        if fbo == 0 {
            let mut current_fbo = 0;

            unsafe {
                epoxy::GetIntegerv(epoxy::FRAMEBUFFER_BINDING, &mut current_fbo);
            }

            fbo = current_fbo as u32;
            self.fbo.set(fbo);
        }

        fbo as i32
    }

    fn on_event<T: Fn(Event)>(&self, callback: T) {
        if let Some(result) = self.mpv.borrow_mut().wait_event(0.0) {
            match result {
                Ok(event) => callback(event),
                Err(e) => error!("Failed to wait for event: {e}"),
            }
        };
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

impl ObjectImpl for Video {
    fn signals() -> &'static [Signal] {
        static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
        SIGNALS.get_or_init(|| {
            vec![
                Signal::builder("property-changed")
                    .param_types([str::static_type(), Variant::static_type()])
                    .build(),
                Signal::builder("playback-started").build(),
                Signal::builder("playback-ended").build(),
            ]
        })
    }

    fn constructed(&self) {
        self.parent_constructed();

        let video_weak = self.downgrade();
        let object_weak = self.obj().downgrade();

        glib::idle_add_local(move || {
            if let Some(video) = video_weak.upgrade()
                && let Some(object) = object_weak.upgrade()
            {
                video.on_event(|event| match event {
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
                    Event::EndFile(_) => {
                        object.emit_by_name::<()>("playback-ended", &[]);
                    }
                    _ => {}
                });
            }

            glib::ControlFlow::Continue
        });
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

            let mut render_context = RenderContext::new(
                mpv_handle,
                vec![
                    RenderParam::ApiType(RenderParamApiType::OpenGl),
                    RenderParam::InitParams(OpenGLInitParams {
                        get_proc_address,
                        ctx: context,
                    }),
                    RenderParam::BlockForTargetTime(false),
                ],
            )
            .expect("Failed to create render context");

            let (sender, receiver) = channel::<()>();

            let object_weak = object.downgrade();
            glib::idle_add_local(move || {
                if let Ok(()) = receiver.try_recv()
                    && let Some(object) = object_weak.upgrade()
                {
                    object.queue_render();
                }

                glib::ControlFlow::Continue
            });

            render_context.set_update_callback(move || {
                sender.send(()).ok();
            });

            *self.render_context.borrow_mut() = Some(render_context);
        }
    }

    fn unrealize(&self) {
        if let Some(render_context) = self.render_context.borrow_mut().take() {
            drop(render_context);
        }

        self.parent_unrealize();
    }
}

impl GLAreaImpl for Video {
    fn render(&self, _context: &GLContext) -> Propagation {
        let object = self.obj();

        let fbo = self.fbo();
        let width = object.width();
        let height = object.height();

        if let Some(ref render_context) = *self.render_context.borrow() {
            render_context
                .render::<GLContext>(fbo, width, height, true)
                .expect("Failed to render");
        }

        Propagation::Stop
    }
}
