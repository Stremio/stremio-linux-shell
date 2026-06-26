use gtk::{
    gdk::{self, GLContext},
    glib::{
        self, Propagation, Properties, SignalHandlerId, Variant, clone, subclass::Signal,
        translate::ToGlibPtr,
    },
    prelude::*,
    subclass::prelude::*,
};
use libc::{LC_NUMERIC, setlocale};
use libmpv2::{
    Format, Mpv, SetData,
    events::{Event, PropertyData},
    render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType, mpv_render_update},
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

fn native_display_param(display: &gdk::Display) -> Option<RenderParam<GLContext>> {
    if let Some(display) = display.downcast_ref::<gdk_wayland::WaylandDisplay>() {
        let wl_display = unsafe {
            gdk_wayland::ffi::gdk_wayland_display_get_wl_display(display.to_glib_none().0)
        };

        if !wl_display.is_null() {
            return Some(RenderParam::WaylandDisplay(wl_display as *const c_void));
        }
    }

    if let Some(display) = display.downcast_ref::<gdk_x11::X11Display>() {
        let x_display =
            unsafe { gdk_x11::ffi::gdk_x11_display_get_xdisplay(display.to_glib_none().0) };

        if !x_display.is_null() {
            return Some(RenderParam::X11Display(x_display as *const c_void));
        }
    }

    None
}

#[derive(Properties)]
#[properties(wrapper_type = super::Video)]
pub struct Video {
    mpv: RefCell<Mpv>,
    render_context: RefCell<Option<RenderContext>>,
    frame_rendered: Cell<bool>,
    frame_clock_handler: RefCell<Option<(gdk::FrameClock, SignalHandlerId)>>,
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
            frame_rendered: Default::default(),
            frame_clock_handler: Default::default(),
        }
    }
}

impl Video {
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
                Signal::builder("playback-ended").build(),
            ]
        })
    }

    fn constructed(&self) {
        self.parent_constructed();

        glib::idle_add_local(clone!(
            #[weak(rename_to = video)]
            self,
            #[weak(rename_to = object)]
            self.obj(),
            #[upgrade_or]
            glib::ControlFlow::Continue,
            move || {
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

                glib::ControlFlow::Continue
            }
        ));
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
                RenderParam::AdvancedControl(true),
            ];

            if let Some(param) = native_display_param(&object.display()) {
                render_params.push(param);
            }

            let mut render_context = RenderContext::new(mpv_handle, render_params)
                .expect("Failed to create render context");

            let (sender, receiver) = channel::<()>();

            glib::idle_add_local(clone!(
                #[weak(rename_to = video)]
                self,
                #[weak]
                object,
                #[upgrade_or]
                glib::ControlFlow::Continue,
                move || {
                    let mut updated = false;
                    while receiver.try_recv().is_ok() {
                        updated = true;
                    }

                    if updated {
                        object.make_current();

                        if object.error().is_none()
                            && let Some(ref render_context) = *video.render_context.borrow()
                        {
                            match render_context.update() {
                                Ok(flags) if flags & mpv_render_update::Frame != 0 => {
                                    object.queue_render();
                                }
                                Ok(_) => {}
                                Err(e) => error!("Failed to update render context: {e}"),
                            }
                        }
                    }

                    glib::ControlFlow::Continue
                }
            ));

            render_context.set_update_callback(move || {
                sender.send(()).ok();
            });

            if let Some((frame_clock, handler)) = self.frame_clock_handler.borrow_mut().take() {
                frame_clock.disconnect(handler);
            }

            if let Some(frame_clock) = object.frame_clock() {
                let handler = frame_clock.connect_after_paint(clone!(
                    #[weak(rename_to = video)]
                    self,
                    #[weak]
                    object,
                    move |_| {
                        if !video.frame_rendered.replace(false) {
                            return;
                        }

                        object.make_current();

                        if object.error().is_none()
                            && let Some(ref render_context) = *video.render_context.borrow()
                        {
                            render_context.report_swap();
                        }
                    }
                ));

                *self.frame_clock_handler.borrow_mut() = Some((frame_clock, handler));
            }

            *self.render_context.borrow_mut() = Some(render_context);
        }
    }

    fn unrealize(&self) {
        if let Some((frame_clock, handler)) = self.frame_clock_handler.borrow_mut().take() {
            frame_clock.disconnect(handler);
        }
        self.frame_rendered.set(false);

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
            match render_context.render::<GLContext>(fbo, width, height, true) {
                Ok(()) => self.frame_rendered.set(true),
                Err(e) => error!("Failed to render: {e}"),
            }
        }

        Propagation::Stop
    }
}
