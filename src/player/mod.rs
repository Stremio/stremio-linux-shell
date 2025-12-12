mod constants;

use std::{collections::HashSet, env, ffi::CString, os::raw::c_void, rc::Rc, sync::RwLock};

use crate::shared::types::UserEvent;
use constants::{BOOL_PROPERTIES, FLOAT_PROPERTIES, STRING_PROPERTIES};
use crossbeam_channel::{Receiver, Sender, unbounded};
use glutin::{display::Display, prelude::GlDisplay};
use itertools::Itertools;
use libc::{LC_NUMERIC, setlocale};
use libmpv2::{
    Format, Mpv,
    events::{Event, EventContext, PropertyData},
    render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType},
};
use rust_i18n::t;
use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};
use serde_json::{Number, Value};
use tracing::error;
use winit::event_loop::EventLoopProxy;

pub type GLContext = Rc<Display>;

#[derive(Debug, Clone)]
pub enum MpvPropertyValue {
    Float(f64),
    Bool(bool),
    String(String),
}

impl Serialize for MpvPropertyValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            MpvPropertyValue::Float(value) => serializer.serialize_f64(*value),
            MpvPropertyValue::Bool(value) => serializer.serialize_bool(*value),
            MpvPropertyValue::String(value) => {
                if let Ok(json_value) = serde_json::from_str::<Value>(value) {
                    json_value.serialize(serializer)
                } else {
                    serializer.serialize_str(value)
                }
            }
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct MpvProperty(pub String, pub Option<Value>);

impl MpvProperty {
    pub fn name(&self) -> &str {
        self.0.as_ref()
    }

    pub fn value(&self) -> Result<MpvPropertyValue, &'static str> {
        if let Some(value) = self.1.clone() {
            if FLOAT_PROPERTIES.contains(&self.name()) {
                return serde_json::from_value::<f64>(value)
                    .map(MpvPropertyValue::Float)
                    .map_err(|_| "Failed to get f64 from Value");
            }

            if BOOL_PROPERTIES.contains(&self.name()) {
                return serde_json::from_value::<bool>(value)
                    .map(MpvPropertyValue::Bool)
                    .map_err(|_| "Failed to get bool from Value");
            }

            if STRING_PROPERTIES.contains(&self.name()) {
                return serde_json::from_value::<String>(value)
                    .map(MpvPropertyValue::String)
                    .map_err(|_| "Failed to get String from Value");
            }
        }

        Err("Failed to get value of MpvProperty")
    }
}

impl Serialize for MpvProperty {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("MpvProperty", 2)?;
        state.serialize_field("name", self.name())?;

        if let Ok(value) = self.value() {
            state.serialize_field("data", &value)?;
        }

        state.end()
    }
}

#[derive(Debug)]
pub enum PlayerEvent {
    Start,
    Stop(Option<String>),
    Update,
    PropertyChange(MpvProperty),
    MpvError(String),
}

impl<'a> TryFrom<Event<'a>> for PlayerEvent {
    type Error = &'static str;

    fn try_from(value: Event<'a>) -> Result<Self, Self::Error> {
        match value {
            Event::StartFile => Ok(PlayerEvent::Start),
            Event::EndFile(code) => {
                let error = match code {
                    3 => Some(t!("player_error_quit")),
                    4 => Some(t!("player_error_general")),
                    _ => None,
                };

                Ok(PlayerEvent::Stop(error.map(String::from)))
            }
            Event::PropertyChange { name, change, .. } => {
                let property = match change {
                    PropertyData::Double(value) => MpvProperty(
                        name.to_owned(),
                        Some(Value::Number(Number::from_f64(value).unwrap())),
                    ),
                    PropertyData::Flag(value) => {
                        MpvProperty(name.to_owned(), Some(Value::Bool(value)))
                    }
                    PropertyData::Str(value) => {
                        MpvProperty(name.to_owned(), Some(Value::String(value.to_owned())))
                    }
                    _ => return Err("Property not supported"),
                };

                Ok(PlayerEvent::PropertyChange(property))
            }
            _ => Err("Event not supported"),
        }
    }
}

pub struct Player {
    mpv: Mpv,
    event_context: EventContext,
    render_context: Option<RenderContext>,
    sender: Sender<PlayerEvent>,
    receiver: Receiver<PlayerEvent>,
    observed: RwLock<HashSet<String>>,
}

impl Player {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
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
            init.set_property("hwdec", "auto")?;
            init.set_property("vd-lavc-dr", "yes")?;
            init.set_property("video-timing-offset", "0")?;
            init.set_property("terminal", "yes")?;
            init.set_property("msg-level", msg_level)?;

            // Performance tuning
            init.set_property("cache", "yes")?;
            init.set_property("demuxer-max-bytes", "100000000")?; // 100MB
            init.set_property("demuxer-readahead-secs", "20")?;

            Ok(())
        })
        .expect("Failed to creating mpv");

        let mut event_context = EventContext::new(mpv.ctx);
        if let Err(e) = event_context.disable_deprecated_events() {
            error!("Failed to disable deprecated events: {}", e);
        }

        event_context.set_wakeup_callback(move || {
            proxy.send_event(UserEvent::MpvEventAvailable).ok();
        });

        let (sender, receiver) = unbounded::<PlayerEvent>();

        Self {
            mpv,
            event_context,
            render_context: None,
            sender,
            receiver,
            observed: RwLock::new(HashSet::new()),
        }
    }

    pub fn setup(&mut self, context: GLContext) {
        self.render_context.take();

        fn get_proc_address(context: &GLContext, name: &str) -> *mut c_void {
            let procname = CString::new(name).unwrap();
            context.get_proc_address(procname.as_c_str()) as _
        }

        let mpv_handle = unsafe { self.mpv.ctx.as_mut() };

        let render_context = RenderContext::new(
            mpv_handle,
            vec![
                RenderParam::ApiType(RenderParamApiType::OpenGl),
                RenderParam::InitParams(OpenGLInitParams {
                    get_proc_address,
                    ctx: context,
                }),
                RenderParam::BlockForTargetTime(false),
                // RenderParam::AdvancedControl(true),
            ],
        );

        if let Ok(mut render_context) =
            render_context.map_err(|e| error!("Failed to create render context: {e}"))
        {
            let sender = self.sender.clone();
            render_context.set_update_callback(move || {
                sender.send(PlayerEvent::Update).ok();
            });
            self.render_context = Some(render_context);
        }
    }

    pub fn render(&self, fbo: u32, width: i32, height: i32) {
        if let Some(render_context) = self.render_context.as_ref()
            && width > 0
            && height > 0
        {
            if let Err(e) = render_context.render::<GLContext>(fbo as i32, width, height, false) {
                error!("Failed to render: {e}");
            }
        }
    }

    pub fn report_swap(&self) {
        if let Some(render_context) = self.render_context.as_ref() {
            render_context.report_swap();
        }
    }

    pub fn events<T: FnMut(PlayerEvent)>(&mut self, mut handler: T) {
        self.receiver.try_iter().for_each(&mut handler);

        let sender = self.sender.clone();

        // Drain events to avoid backlog
        loop {
            if let Some(result) = self.event_context.wait_event(0.0) {
                match result {
                    Ok(event) => {
                        if let Ok(player_event) = PlayerEvent::try_from(event) {
                            sender.send(player_event).ok();
                        }
                    }
                    Err(e) => {
                        error!("Mpv error: {e}");
                        sender.send(PlayerEvent::MpvError(e.to_string())).ok();
                    }
                }
            } else {
                break;
            }
        }

        self.receiver.try_iter().for_each(handler);
    }

    pub fn command(&self, name: String, args: Vec<String>) {
        let args = args.iter().map(String::as_ref).collect_vec();
        if let Err(e) = self.mpv.command(&name, &args) {
            error!("Failed to use command {name}: {e}");
        }
    }

    pub fn observe_property(&self, name: String) {
        let format = match name.as_str() {
            name if FLOAT_PROPERTIES.contains(&name) => Some(Format::Double),
            name if BOOL_PROPERTIES.contains(&name) => Some(Format::Flag),
            name if STRING_PROPERTIES.contains(&name) => Some(Format::String),
            name if STRING_PROPERTIES.contains(&name) => Some(Format::String),
            _ => None,
        };

        if let Some(format) = format
            && !self.observed.read().unwrap().contains(&name)
            && let Ok(mut observed) = self.observed.write()
        {
            if let Err(e) = self.event_context.observe_property(&name, format, 0) {
                error!("Failed to observe property {name}: {e}");
            } else {
                observed.insert(name);
            }
        }
    }

    pub fn set_property(&self, property: MpvProperty) {
        match property.name() {
            name if FLOAT_PROPERTIES.contains(&name) => {
                if let Ok(MpvPropertyValue::Float(value)) = property.value()
                    && let Err(e) = self.mpv.set_property(name, value)
                {
                    error!("Failed to set property {name}: {e}");
                }
            }
            name if BOOL_PROPERTIES.contains(&name) => {
                if let Ok(MpvPropertyValue::Bool(value)) = property.value()
                    && let Err(e) = self.mpv.set_property(name, value)
                {
                    error!("Failed to set property {name}: {e}");
                }
            }
            name if STRING_PROPERTIES.contains(&name) => {
                if let Ok(MpvPropertyValue::String(value)) = property.value()
                    && let Err(e) = self.mpv.set_property(name, value)
                {
                    error!("Failed to set property {name}: {e}");
                }
            }
            name => error!("Failed to set property {name}: Unsupported"),
        };
    }

    pub fn release(&mut self) {
        self.render_context.take();
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.render_context.take();
    }
}
