use gdk_wayland::{WaylandDisplay, wayland_client::Proxy};
use gtk::{
    gdk::GLContext,
    glib::{self, Propagation, Properties, Variant, subclass::Signal},
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
    cell::RefCell,
    env,
    os::raw::c_void,
    ptr::NonNull,
    sync::{Arc, Mutex, OnceLock},
    thread::{self, JoinHandle},
};
use tracing::error;

use crate::spawn_local;

fn get_proc_address(_context: &GLContext, name: &str) -> *mut c_void {
    epoxy::get_proc_addr(name) as _
}

enum VideoEvent {
    PropertyChanged(String, Variant),
    PlaybackEnded(&'static str),
}

enum EventLoopCommand {
    Observe(String, Format),
    Shutdown,
}

struct MpvEventLoop {
    commands: flume::Sender<EventLoopCommand>,
    _client_keepalive: Arc<Mutex<Mpv>>,
    context_ptr: NonNull<libmpv2_sys::mpv_handle>,
    thread: Option<JoinHandle<()>>,
}

impl MpvEventLoop {
    fn new(mpv: Mpv) -> (Self, flume::Receiver<VideoEvent>) {
        let (command_sender, command_receiver) = flume::unbounded();
        let (event_sender, event_receiver) = flume::unbounded();
        let context_ptr = mpv.ctx;
        let context = Arc::new(Mutex::new(mpv));
        let thread_context = context.clone();

        let thread = thread::Builder::new()
            .name("mpv-events".to_string())
            .spawn(move || {
                let mut mpv = thread_context
                    .lock()
                    .expect("Failed to lock mpv event client");

                loop {
                    for command in command_receiver.try_iter() {
                        match command {
                            EventLoopCommand::Observe(name, format) => {
                                if let Err(e) = mpv.observe_property(&name, format, 0) {
                                    error!("Failed to observe property {name}: {e}");
                                }
                            }
                            EventLoopCommand::Shutdown => return,
                        }
                    }

                    let event = match mpv.wait_event(-1.0) {
                        Some(Ok(event)) => event,
                        Some(Err(e)) => {
                            error!("Failed to wait for event: {e}");
                            continue;
                        }
                        None => continue,
                    };

                    let event = match event {
                        Event::PropertyChange { name, change, .. } => {
                            let value = match change {
                                PropertyData::Str(value) => Some(value.to_variant()),
                                PropertyData::Flag(value) => Some(value.to_variant()),
                                PropertyData::Double(value) => Some(value.to_variant()),
                                _ => None,
                            };

                            value.map(|value| VideoEvent::PropertyChanged(name.to_string(), value))
                        }
                        Event::EndFile(reason) => {
                            let reason = match reason {
                                mpv_end_file_reason::Eof => "eof",
                                mpv_end_file_reason::Stop => "stop",
                                mpv_end_file_reason::Redirect => "redirect",
                                mpv_end_file_reason::Error => "error",
                                mpv_end_file_reason::Quit => "quit",
                                _ => "other",
                            };

                            Some(VideoEvent::PlaybackEnded(reason))
                        }
                        Event::Shutdown => break,
                        _ => None,
                    };

                    if let Some(event) = event
                        && event_sender.send(event).is_err()
                    {
                        break;
                    }
                }
            })
            .expect("Failed to create mpv event thread");

        (
            Self {
                commands: command_sender,
                _client_keepalive: context,
                context_ptr,
                thread: Some(thread),
            },
            event_receiver,
        )
    }

    fn observe_property(&self, name: &str, format: Format) {
        match self
            .commands
            .send(EventLoopCommand::Observe(name.to_string(), format))
        {
            Ok(()) => self.wakeup(),
            Err(_) => error!("Failed to observe property {name}: event loop stopped"),
        }
    }

    fn wakeup(&self) {
        unsafe {
            libmpv2_sys::mpv_wakeup(self.context_ptr.as_ptr());
        }
    }
}

impl Drop for MpvEventLoop {
    fn drop(&mut self) {
        self.commands.send(EventLoopCommand::Shutdown).ok();
        self.wakeup();

        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            error!("Mpv event thread panicked");
        }
    }
}

#[derive(Properties)]
#[properties(wrapper_type = super::Video)]
pub struct Video {
    mpv: RefCell<Mpv>,
    event_loop: MpvEventLoop,
    event_receiver: flume::Receiver<VideoEvent>,
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
            init.set_property("terminal", "yes")?;
            init.set_property("msg-level", msg_level)?;
            Ok(())
        })
        .expect("Failed to create mpv");

        let event_client = mpv
            .create_client(Some("stremio_events"))
            .expect("Failed to create mpv event client");
        mpv.disable_deprecated_events().ok();
        for event in 2..26 {
            mpv.disable_event(event).ok();
        }

        let (event_loop, event_receiver) = MpvEventLoop::new(event_client);

        Self {
            mpv: RefCell::new(mpv),
            event_loop,
            event_receiver,
            render_context: Default::default(),
        }
    }
}

impl Video {
    pub fn send_command(&self, name: &str, args: &[&str]) {
        if let Err(e) = self.mpv.borrow().command(name, args) {
            error!("Failed to send command {name}: {e}");
        }
    }

    pub fn observe_property(&self, name: &str, format: Format) {
        self.event_loop.observe_property(name, format);
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
                Signal::builder("playback-ended")
                    .param_types([str::static_type()])
                    .build(),
            ]
        })
    }

    fn constructed(&self) {
        self.parent_constructed();

        let receiver = self.event_receiver.clone();
        let object = self.obj().downgrade();

        spawn_local!(async move {
            while let Ok(event) = receiver.recv_async().await {
                let Some(object) = object.upgrade() else {
                    break;
                };

                match event {
                    VideoEvent::PropertyChanged(name, value) => {
                        object.emit_by_name::<()>("property-changed", &[&name, &value]);
                    }
                    VideoEvent::PlaybackEnded(reason) => {
                        object.emit_by_name::<()>("playback-ended", &[&reason]);
                    }
                }
            }
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

            let (sender, receiver) = flume::bounded::<()>(1);

            let weak_object = object.downgrade();

            spawn_local!(async move {
                while receiver.recv_async().await.is_ok() {
                    let Some(object) = weak_object.upgrade() else {
                        break;
                    };

                    object.queue_render();
                }
            });

            render_context.set_update_callback(move || {
                sender.try_send(()).ok();
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
