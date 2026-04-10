//! This is still in early development, so it may not be fully functioning yet.
//!
//! It will show on Discord what you're streaming
//! Masquerade (cmdmasquerade)

use discord_rich_presence::{
    DiscordIpc, DiscordIpcClient,
    activity::{Activity, Timestamps},
};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::warn;

const APP_ID: &str = "1492012722076909629";

pub struct Discord {
    client: Option<DiscordIpcClient>,
    start_time: Option<i64>,
}

impl Discord {
    pub fn new() -> Self {
        let mut c = DiscordIpcClient::new(APP_ID);
        let client = match c.connect() {
            Ok(()) => Some(c),
            Err(e) => {
                warn!("Discord RPC unavailable: {e}");
                None
            }
        };

        Self {
            client,
            start_time: None,
        }
    }

    pub fn set_activity(&mut self, title: &str) {
        let Some(client) = self.client.as_mut() else {
            return;
        };

        let start = *self.start_time.get_or_insert_with(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
        });

        let title = if title.is_empty() { "Stremio" } else { title };

        if let Err(e) = client.set_activity(
            Activity::new()
                .details(title)
                .state("Watching on Stremio")
                .timestamps(Timestamps::new().start(start)),
        ) {
            warn!("Discord RPC set_activity failed: {e}");
        }
    }

    pub fn clear_activity(&mut self) {
        self.start_time = None;

        let Some(client) = self.client.as_mut() else {
            return;
        };

        if let Err(e) = client.clear_activity() {
            warn!("Discord RPC clear_activity failed: {e}");
        }
    }
}
