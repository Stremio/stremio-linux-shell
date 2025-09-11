use serde::Deserialize;
use serde_json::Value;

use crate::app::ipc::event::{IpcEvent, IpcEventMpv};

#[derive(Deserialize, Debug)]
pub struct IpcMessageRequestWinSetVisilibty {
    fullscreen: bool,
}

#[derive(Deserialize, Debug)]
pub struct IpcMessageRequest {
    r#type: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<serde_json::Value>,
}

impl TryFrom<IpcMessageRequest> for IpcEvent {
    type Error = &'static str;

    fn try_from(value: IpcMessageRequest) -> Result<Self, Self::Error> {
        match value.r#type {
            3 => Ok(IpcEvent::Init),
            6 => match value.args {
                Some(args) => {
                    let args: Vec<Value> = serde_json::from_value(args).expect("Invalid arguments");
                    let name = args.first().and_then(Value::as_str).ok_or("Invalid name")?;
                    let data = args.get(1).cloned();

                    match data {
                        Some(data) => match name {
                            "app-ready" => Ok(IpcEvent::Ready),
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

                                let value = key_value
                                    .get(1)
                                    .expect("Invalid mpv-set-prop value")
                                    .to_owned();

                                Ok(IpcEvent::Mpv(IpcEventMpv::Set((name, value))))
                            }
                            _ => Err("Unknown method"),
                        },
                        None => match name {
                            "quit" => Ok(IpcEvent::Quit),
                            _ => Err("Unknown method"),
                        },
                    }
                }
                None => Err("Missing args"),
            },
            _ => Err("Unknown type"),
        }
    }
}
