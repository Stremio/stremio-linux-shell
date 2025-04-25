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
}

#[derive(Deserialize, Debug)]
pub enum IpcEvent {
    Init(u64),
    Fullscreen(bool),
    OpenMedia(String),
    Mpv(IpcEventMpv),
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

    fn try_from(value: IpcMessageRequest) -> Result<Self, Self::Error> {
        match value.r#type {
            3 => Ok(IpcEvent::Init(value.id)),
            6 => match value.args {
                Some(args) => {
                    let args: Vec<Value> = serde_json::from_value(args).expect("Invalid arguments");
                    let name = args.first().and_then(Value::as_str).ok_or("Invalid name")?;
                    let data = args.get(1).cloned().ok_or("Missing data")?;

                    match name {
                        "win-set-visibility" => {
                            let data: IpcMessageRequestWinSetVisilibty =
                                serde_json::from_value(data)
                                    .expect("Invalid win-set-visibility object");

                            Ok(IpcEvent::Fullscreen(data.fullscreen))
                        }
                        "mpv-command" => {
                            let data: Vec<String> = serde_json::from_value(data)
                                .expect("Invalid mpv-command arguments");
                            let name = data[0].clone();

                            let mut args = vec![];
                            for arg in data.iter().skip(1) {
                                args.push(arg.clone());
                            }

                            Ok(IpcEvent::Mpv(IpcEventMpv::Command((name, args))))
                        }
                        "mpv-observe-prop" => {
                            let name = data.as_str().expect("Invalid mpv-observe-prop name");
                            Ok(IpcEvent::Mpv(IpcEventMpv::Observe(name.to_owned())))
                        }
                        "mpv-set-prop" => {
                            let key_value: Vec<Value> = serde_json::from_value(data)
                                .expect("Invalid mpv-set-prop arguments");

                            let name = key_value[0]
                                .as_str()
                                .expect("Invalid mpv-set-prop name")
                                .to_owned();

                            let value = key_value.get(1).cloned();

                            Ok(IpcEvent::Mpv(IpcEventMpv::Set(MpvProperty(name, value))))
                        }
                        _ => Err("Unknown method"),
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

#[derive(Serialize, Debug, Clone)]
pub struct IpcMessageResponseDataTransport {
    properties: Vec<Vec<String>>,
    signals: Vec<String>,
    methods: Vec<Vec<String>>,
}

#[derive(Serialize, Debug, Clone)]
pub struct IpcMessageResponseData {
    transport: IpcMessageResponseDataTransport,
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
            _ => Err("Failed to convert IpcEvent to IpcMessageResponse"),
        }
    }
}

pub fn parse_request<T: Fn(IpcEvent)>(data: String, handler: T) {
    IpcEvent::try_from(data)
        .map(handler)
        .map_err(|e| eprintln!("{e}"))
        .ok();
}

pub fn create_response(event: IpcEvent) -> String {
    let message = IpcMessageResponse::try_from(event).ok();
    serde_json::to_string(&message).expect("Failed to convert IpcMessage to string")
}
