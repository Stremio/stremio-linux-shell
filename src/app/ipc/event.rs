use serde::Deserialize;
use serde_json::Value;

use super::request::IpcMessageRequest;

#[derive(Deserialize, Debug)]
pub enum IpcEventMpv {
    Observe(String),
    Command((String, Vec<String>)),
    Set((String, Value)),
    Change((String, Value)),
    Ended(Option<String>),
}

#[derive(Deserialize, Debug)]
pub enum IpcEvent {
    Init,
    Ready,
    Quit,
    Fullscreen(bool),
    Visibility(bool),
    OpenMedia(String),
    Mpv(IpcEventMpv),
}

impl TryFrom<&str> for IpcEvent {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        serde_json::from_str::<IpcMessageRequest>(value)
            .map_err(|e| format!("Failed to convert String to IpcEvent: {e}"))?
            .try_into()
            .map_err(|e| format!("Failed to convert IpcEvent to IpcMessageRequest: {e}"))
    }
}
