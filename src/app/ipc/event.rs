use serde::Deserialize;
use serde_json::Value;

use super::request::IpcMessageRequest;

#[derive(Deserialize, Debug)]
pub enum IpcEventMpv {
    Observe(String),
    Command((String, Vec<String>)),
    Set((String, Value)),
    Change((String, Value)),
    Ended((String, Option<String>)),
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DiscordActivity {
    pub state: String,
    pub details: String,
    pub image: Option<String>,
    pub start_timestamp: Option<i64>,
    pub end_timestamp: Option<i64>,
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
    MediaMetadata((String, Option<String>, Option<String>)),
    MediaStatus(bool),
    DiscordConnect,
    DiscordDisconnect,
    DiscordSetActivity(DiscordActivity),
    DiscordClearActivity,
    DiscordStatus(bool),
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
