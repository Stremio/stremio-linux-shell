use serde::Serialize;
use serde_json::json;

use super::{
    TRANSPORT_NAME, VERSION,
    event::{IpcEvent, IpcEventMpv},
};

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
            IpcEvent::Init => Ok(IpcMessageResponse {
                id: 0,
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
            IpcEvent::OpenMedia(deeplink) => Ok(IpcMessageResponse {
                id: 1,
                r#type: 1,
                object: TRANSPORT_NAME.to_owned(),
                data: None,
                args: Some(json!(["open-media", deeplink])),
            }),
            IpcEvent::Mpv(IpcEventMpv::Change((name, value))) => Ok(IpcMessageResponse {
                id: 1,
                r#type: 1,
                object: TRANSPORT_NAME.to_owned(),
                data: None,
                args: Some(json!(["mpv-prop-change", {
                    "name": name,
                    "data": value,
                }])),
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
            _ => Err("Failed to convert IpcEvent to IpcMessageResponse"),
        }
    }
}
