use serde::Deserialize;
use serde_json::Value;

use super::event::{DiscordActivity, IpcEvent, IpcEventMpv};

#[derive(Deserialize, Debug)]
pub struct IpcMessageRequest {
    r#type: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
pub struct IpcMessageRequestWinSetVisilibty {
    fullscreen: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct IpcMessageRequestMediaMetadata {
    title: String,
    artist: Option<String>,
    art_url: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct IpcMessageRequestMediaStatus {
    paused: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct IpcMessageRequestDiscordSetActivity {
    state: String,
    details: String,
    image: Option<String>,
    start_timestamp: Option<i64>,
    end_timestamp: Option<i64>,
}

impl From<IpcMessageRequestDiscordSetActivity> for DiscordActivity {
    fn from(value: IpcMessageRequestDiscordSetActivity) -> Self {
        Self {
            state: value.state,
            details: value.details,
            image: value.image,
            start_timestamp: value.start_timestamp,
            end_timestamp: value.end_timestamp,
        }
    }
}

impl TryFrom<IpcMessageRequest> for IpcEvent {
    type Error = String;

    fn try_from(value: IpcMessageRequest) -> Result<Self, Self::Error> {
        match value.r#type {
            3 => Ok(IpcEvent::Init),
            6 => match value.args {
                Some(args) => {
                    let args: Vec<Value> =
                        serde_json::from_value(args).map_err(|_| "Invalid arguments")?;
                    let name = args.first().and_then(Value::as_str).ok_or("Invalid name")?;
                    let data = args.get(1).cloned();

                    match data {
                        Some(data) => match name {
                            "win-set-visibility" => {
                                let data: IpcMessageRequestWinSetVisilibty =
                                    serde_json::from_value(data)
                                        .map_err(|_| "Invalid win-set-visibility object")?;

                                Ok(IpcEvent::Fullscreen(data.fullscreen))
                            }
                            "mpv-command" => {
                                let data: Vec<String> = serde_json::from_value(data)
                                    .map_err(|_| "Invalid mpv-command arguments")?;
                                let name = data[0].clone();

                                let mut args = vec![];
                                for arg in data.iter().skip(1) {
                                    args.push(arg.clone());
                                }

                                Ok(IpcEvent::Mpv(IpcEventMpv::Command((name, args))))
                            }
                            "mpv-observe-prop" => {
                                let name = data.as_str().ok_or("Invalid mpv-observe-prop name")?;
                                Ok(IpcEvent::Mpv(IpcEventMpv::Observe(name.to_owned())))
                            }
                            "mpv-set-prop" => {
                                let key_value: Vec<Value> = serde_json::from_value(data)
                                    .map_err(|_| "Invalid mpv-set-prop arguments")?;

                                let name = key_value[0]
                                    .as_str()
                                    .ok_or("Invalid mpv-set-prop name")?
                                    .to_owned();

                                let value = key_value
                                    .get(1)
                                    .ok_or("Invalid mpv-set-prop value")?
                                    .to_owned();

                                Ok(IpcEvent::Mpv(IpcEventMpv::Set((name, value))))
                            }
                            "media.metadata" => {
                                let data: IpcMessageRequestMediaMetadata =
                                    serde_json::from_value(data)
                                        .map_err(|_| "Invalid media.metadata object")?;

                                Ok(IpcEvent::MediaMetadata((
                                    data.title,
                                    data.artist,
                                    data.art_url,
                                )))
                            }
                            "media.status" => {
                                let data: IpcMessageRequestMediaStatus =
                                    serde_json::from_value(data)
                                        .map_err(|_| "Invalid media.status object")?;

                                Ok(IpcEvent::MediaStatus(data.paused))
                            }
                            "discord-connect" => Ok(IpcEvent::DiscordConnect),
                            "discord-disconnect" => Ok(IpcEvent::DiscordDisconnect),
                            "discord-set-activity" => {
                                let data: IpcMessageRequestDiscordSetActivity =
                                    serde_json::from_value(data)
                                        .map_err(|_| "Invalid discord-set-activity object")?;

                                Ok(IpcEvent::DiscordSetActivity(data.into()))
                            }
                            "discord-clear-activity" => Ok(IpcEvent::DiscordClearActivity),
                            method => Err(format!("Invalid method: {method}")),
                        },
                        None => match name {
                            "app-ready" => Ok(IpcEvent::Ready),
                            "quit" => Ok(IpcEvent::Quit),
                            method => Err(format!("Invalid method: {method}")),
                        },
                    }
                }
                None => Err("Invalid arguments".into()),
            },
            r#type => Err(format!("Invalid type: {}", r#type)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(message: &str) -> Result<IpcEvent, String> {
        IpcEvent::try_from(message)
    }

    #[test]
    fn parses_discord_connect() {
        let event = parse(r#"{"type":6,"args":["discord-connect",{}]}"#);

        assert!(matches!(event, Ok(IpcEvent::DiscordConnect)));
    }

    #[test]
    fn parses_discord_disconnect() {
        let event = parse(r#"{"type":6,"args":["discord-disconnect",{}]}"#);

        assert!(matches!(event, Ok(IpcEvent::DiscordDisconnect)));
    }

    #[test]
    fn parses_discord_clear_activity() {
        let event = parse(r#"{"type":6,"args":["discord-clear-activity",{}]}"#);

        assert!(matches!(event, Ok(IpcEvent::DiscordClearActivity)));
    }

    #[test]
    fn parses_discord_set_activity() {
        let event = parse(
            r#"{"type":6,"args":["discord-set-activity",{
                "state": "Watching",
                "details": "Movie",
                "image": "https://example.com/poster.jpg",
                "startTimestamp": 1752700000,
                "endTimestamp": 1752707200
            }]}"#,
        );

        let Ok(IpcEvent::DiscordSetActivity(activity)) = event else {
            panic!("Expected DiscordSetActivity, got {event:?}");
        };

        assert_eq!(activity.state, "Watching");
        assert_eq!(activity.details, "Movie");
        assert_eq!(
            activity.image.as_deref(),
            Some("https://example.com/poster.jpg")
        );
        assert_eq!(activity.start_timestamp, Some(1752700000));
        assert_eq!(activity.end_timestamp, Some(1752707200));
    }

    #[test]
    fn parses_discord_set_activity_with_null_optional_fields() {
        let event = parse(
            r#"{"type":6,"args":["discord-set-activity",{
                "state": "Paused",
                "details": "Episode",
                "image": null,
                "startTimestamp": null,
                "endTimestamp": null
            }]}"#,
        );

        let Ok(IpcEvent::DiscordSetActivity(activity)) = event else {
            panic!("Expected DiscordSetActivity, got {event:?}");
        };

        assert_eq!(activity.state, "Paused");
        assert_eq!(activity.details, "Episode");
        assert_eq!(activity.image, None);
        assert_eq!(activity.start_timestamp, None);
        assert_eq!(activity.end_timestamp, None);
    }

    #[test]
    fn parses_discord_set_activity_with_missing_optional_fields() {
        let event = parse(
            r#"{"type":6,"args":["discord-set-activity",{
                "state": "Watching",
                "details": "Movie"
            }]}"#,
        );

        let Ok(IpcEvent::DiscordSetActivity(activity)) = event else {
            panic!("Expected DiscordSetActivity, got {event:?}");
        };

        assert_eq!(activity.image, None);
        assert_eq!(activity.start_timestamp, None);
        assert_eq!(activity.end_timestamp, None);
    }

    #[test]
    fn rejects_discord_set_activity_with_missing_required_fields() {
        let without_state =
            parse(r#"{"type":6,"args":["discord-set-activity",{"details": "Movie"}]}"#);
        let without_details =
            parse(r#"{"type":6,"args":["discord-set-activity",{"state": "Watching"}]}"#);

        assert_eq!(
            without_state.err(),
            Some(
                "Failed to convert IpcEvent to IpcMessageRequest: Invalid discord-set-activity object"
                    .to_owned()
            )
        );
        assert_eq!(
            without_details.err(),
            Some(
                "Failed to convert IpcEvent to IpcMessageRequest: Invalid discord-set-activity object"
                    .to_owned()
            )
        );
    }

    #[test]
    fn rejects_discord_set_activity_with_invalid_field_types() {
        let invalid_state = parse(
            r#"{"type":6,"args":["discord-set-activity",{"state": 42, "details": "Movie"}]}"#,
        );
        let invalid_image = parse(
            r#"{"type":6,"args":["discord-set-activity",{"state": "Watching", "details": "Movie", "image": 42}]}"#,
        );
        let invalid_timestamp = parse(
            r#"{"type":6,"args":["discord-set-activity",{"state": "Watching", "details": "Movie", "startTimestamp": "now"}]}"#,
        );

        assert_eq!(
            invalid_state.err(),
            Some(
                "Failed to convert IpcEvent to IpcMessageRequest: Invalid discord-set-activity object"
                    .to_owned()
            )
        );
        assert_eq!(
            invalid_image.err(),
            Some(
                "Failed to convert IpcEvent to IpcMessageRequest: Invalid discord-set-activity object"
                    .to_owned()
            )
        );
        assert_eq!(
            invalid_timestamp.err(),
            Some(
                "Failed to convert IpcEvent to IpcMessageRequest: Invalid discord-set-activity object"
                    .to_owned()
            )
        );
    }

    #[test]
    fn rejects_discord_methods_without_payload() {
        let event = parse(r#"{"type":6,"args":["discord-connect"]}"#);

        assert_eq!(
            event.err(),
            Some(
                "Failed to convert IpcEvent to IpcMessageRequest: Invalid method: discord-connect"
                    .to_owned()
            )
        );
    }

    #[test]
    fn rejects_unknown_discord_method() {
        let event = parse(r#"{"type":6,"args":["discord-status",{}]}"#);

        assert_eq!(
            event.err(),
            Some(
                "Failed to convert IpcEvent to IpcMessageRequest: Invalid method: discord-status"
                    .to_owned()
            )
        );
    }
}
