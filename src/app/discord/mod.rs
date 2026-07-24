mod config;

use std::cell::RefCell;

use discord_rich_presence::{
    DiscordIpc, DiscordIpcClient,
    activity::{Activity, ActivityType, Assets},
};
use tracing::error;

use config::CLIENT_ID;

pub struct Discord {
    client: RefCell<DiscordIpcClient>,
}

impl Discord {
    pub fn new() -> Self {
        let client = RefCell::new(DiscordIpcClient::new(CLIENT_ID));

        Self { client }
    }

    pub fn connect(&self) -> bool {
        if let Err(e) = self.client.borrow_mut().connect() {
            error!("Failed to connect: {e}");
            return false;
        }

        true
    }

    pub fn disconnect(&self) {
        if let Err(e) = self.client.borrow_mut().close() {
            error!("Failed to disconnect: {e}");
        }
    }

    pub fn set_activity(&self, details: String, state: String, image: Option<String>) {
        let mut assets = Assets::new().large_text("Stremio");

        if let Some(image) = image {
            assets = assets.large_image(image);
        }

        let activity = Activity::default()
            .activity_type(ActivityType::Watching)
            .details(details)
            .state(state)
            .assets(assets);

        if let Err(e) = self.client.borrow_mut().set_activity(activity) {
            error!("Failed to set activity: {e}");
        }
    }

    pub fn clear_activity(&self) {
        if let Err(e) = self.client.borrow_mut().clear_activity() {
            error!("Failed to clear activity: {e}");
        }
    }
}
