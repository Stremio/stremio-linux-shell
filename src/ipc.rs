use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::player::MpvProperty;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const TRANSPORT_NAME: &str = "transport";

#[derive(Deserialize, Debug)]
pub enum IpcEventMpv {
    Observe(String),
    Command((String, Vec<String>)),
    Set(MpvProperty),
    Change(MpvProperty),
    Ended(Option<String>),
    Error(String),
}

#[derive(Deserialize, Debug)]
pub enum IpcEvent {
    Init(u64),
    Quit,
    AppReady,
    ReadClipboard,
    Fullscreen(bool),
    Minimized(bool),
    Visibility(bool),
    OpenMedia(String),
    OpenExternal(String),
    Mpv(IpcEventMpv),
    GpuWarning(String),
    NextVideo,
    PreviousVideo,
}

#[derive(Deserialize, Debug)]
pub struct IpcMessageRequestWinSetVisilibty {
    fullscreen: bool,
}

#[derive(Deserialize, Debug)]
pub struct IpcMessageRequest {
    id: u64,
    r#type: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<serde_json::Value>,
}

impl TryFrom<IpcMessageRequest> for IpcEvent {
    type Error = &'static str;

    fn try_from(mut value: IpcMessageRequest) -> Result<Self, Self::Error> {
        match value.r#type {
            3 => Ok(IpcEvent::Init(value.id)),
            6 => match value.args.take() {
                Some(args) => {
                    let mut args: Vec<Value> =
                        serde_json::from_value(args).expect("Invalid arguments");
                    if args.is_empty() {
                        return Err("Invalid name");
                    }
                    let name = match args.remove(0) {
                        Value::String(s) => s,
                        _ => return Err("Invalid name type"),
                    };
                    let data = if !args.is_empty() {
                        Some(args.remove(0))
                    } else {
                        None
                    };

                    match data {
                        Some(data) => match name.as_str() {
                            "win-set-visibility" => {
                                let data: IpcMessageRequestWinSetVisilibty =
                                    serde_json::from_value(data)
                                        .expect("Invalid win-set-visibility object");

                                Ok(IpcEvent::Fullscreen(data.fullscreen))
                            }
                            "open-external" => {
                                let data: String = serde_json::from_value(data)
                                    .expect("Invalid open-external argument");

                                Ok(IpcEvent::OpenExternal(data))
                            }
                            "mpv-command" => {
                                let data: Vec<String> = serde_json::from_value(data)
                                    .expect("Invalid mpv-command arguments");
                                let mut iter = data.into_iter();
                                let name = iter.next().ok_or("Invalid mpv-command name")?;
                                let args = iter.collect();

                                Ok(IpcEvent::Mpv(IpcEventMpv::Command((name, args))))
                            }
                            "mpv-observe-prop" => {
                                let name = match data {
                                    Value::String(s) => s,
                                    _ => return Err("Invalid mpv-observe-prop name"),
                                };
                                Ok(IpcEvent::Mpv(IpcEventMpv::Observe(name)))
                            }
                            "mpv-set-prop" => {
                                let key_value: Vec<Value> = serde_json::from_value(data)
                                    .expect("Invalid mpv-set-prop arguments");
                                let mut iter = key_value.into_iter();

                                let name = match iter.next() {
                                    Some(Value::String(s)) => s,
                                    _ => return Err("Invalid mpv-set-prop name"),
                                };

                                let value = iter.next();

                                Ok(IpcEvent::Mpv(IpcEventMpv::Set(MpvProperty(name, value))))
                            }
                            // Handle app-ready case-insensitively and trim
                            s if s.trim() == "app-ready" => Ok(IpcEvent::AppReady),
                            _ => {
                                eprintln!("Unknown IPC method with data: {}", name);
                                Err("Unknown method")
                            }
                        },
                        None => match name.as_str() {
                            "quit" => Ok(IpcEvent::Quit),
                            "app-ready" => Ok(IpcEvent::AppReady),
                            "read-clipboard" => Ok(IpcEvent::ReadClipboard),
                            _ => {
                                eprintln!("Unknown IPC method without data: {}", name);
                                Err("Unknown method")
                            }
                        },
                    }
                }
                None => Err("Missing args"),
            },
            _ => Err("Unknown type"),
        }
    }
}

impl TryFrom<String> for IpcEvent {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        serde_json::from_str::<IpcMessageRequest>(&value)
            .map_err(|e| format!("Failed to convert String to IpcEvent: {e}"))?
            .try_into()
            .map_err(|e| format!("Failed to convert IpcEvent to IpcMessageRequest: {e}"))
    }
}

#[derive(Serialize, Debug)]
pub struct IpcMessageResponse {
    id: u64,
    r#type: u8,
    object: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<serde_json::Value>,
}

impl TryFrom<IpcEvent> for IpcMessageResponse {
    type Error = &'static str;

    fn try_from(value: IpcEvent) -> Result<Self, Self::Error> {
        match value {
            IpcEvent::Init(id) => Ok(IpcMessageResponse {
                id,
                r#type: 3,
                object: TRANSPORT_NAME.to_owned(),
                args: None,
                data: Some(json!({
                    "transport": {
                        "properties": [[], ["", "shellVersion", "", VERSION]],
                        "signals": [],
                        "methods": [["onEvent"]]
                    }
                })),
            }),
            IpcEvent::Fullscreen(state) => Ok(IpcMessageResponse {
                id: 1,
                r#type: 1,
                object: TRANSPORT_NAME.to_owned(),
                data: None,
                args: Some(json!([
                    "win-visibility-changed",
                    {
                        "visible": true,
                        "visibility": 1,
                        "isFullscreen": state,
                    }
                ])),
            }),
            IpcEvent::Visibility(state) => Ok(IpcMessageResponse {
                id: 1,
                r#type: 1,
                object: TRANSPORT_NAME.to_owned(),
                data: None,
                args: Some(json!([
                    "win-visibility-changed",
                    {
                        "visible": state,
                        "visibility": state as u32,
                        "isFullscreen": false,
                    }
                ])),
            }),
            IpcEvent::Minimized(state) => Ok(IpcMessageResponse {
                id: 1,
                r#type: 1,
                object: TRANSPORT_NAME.to_owned(),
                data: None,
                args: Some(json!([
                    "win-state-changed",
                    {
                        "state": match state {
                            true => 9,
                            false => 8,
                        },
                    }
                ])),
            }),
            IpcEvent::OpenMedia(deeplink) => Ok(IpcMessageResponse {
                id: 1,
                r#type: 1,
                object: TRANSPORT_NAME.to_owned(),
                data: None,
                args: Some(json!(["open-media", deeplink])),
            }),
            IpcEvent::Mpv(IpcEventMpv::Change(property)) => Ok(IpcMessageResponse {
                id: 1,
                r#type: 1,
                object: TRANSPORT_NAME.to_owned(),
                data: None,
                args: Some(json!(["mpv-prop-change", property])),
            }),
            IpcEvent::Mpv(IpcEventMpv::Ended(error)) => Ok(IpcMessageResponse {
                id: 1,
                r#type: 1,
                object: TRANSPORT_NAME.to_owned(),
                data: None,
                args: Some(json!([
                    "mpv-event-ended",
                    {
                        "error": error,
                    }
                ])),
            }),
            IpcEvent::Mpv(IpcEventMpv::Error(error)) => Ok(IpcMessageResponse {
                id: 1,
                r#type: 1,
                object: TRANSPORT_NAME.to_owned(),
                data: None,
                args: Some(json!([
                    "mpv-event-error",
                    {
                        "error": error,
                    }
                ])),
            }),
            IpcEvent::GpuWarning(message) => Ok(IpcMessageResponse {
                id: 1,
                r#type: 1,
                object: TRANSPORT_NAME.to_owned(),
                data: None,
                args: Some(json!(["gpu-warning", message])), // "gpu-warning" will be handled by UI
            }),
            IpcEvent::NextVideo => Ok(IpcMessageResponse {
                id: 1,
                r#type: 1,
                object: TRANSPORT_NAME.to_owned(),
                data: None,
                args: Some(json!(["next-video"])),
            }),
            IpcEvent::PreviousVideo => Ok(IpcMessageResponse {
                id: 1,
                r#type: 1,
                object: TRANSPORT_NAME.to_owned(),
                data: None,
                args: Some(json!(["previous-video"])),
            }),
            _ => Err("Failed to convert IpcEvent to IpcMessageResponse"),
        }
    }
}

pub fn parse_request<T: FnMut(IpcEvent)>(data: String, handler: T) {
    IpcEvent::try_from(data)
        .map(handler)
        .map_err(|e| eprintln!("{e}"))
        .ok();
}

pub fn create_response(event: IpcEvent) -> String {
    let message = IpcMessageResponse::try_from(event).ok();
    serde_json::to_string(&message).expect("Failed to convert IpcMessage to string")
}
