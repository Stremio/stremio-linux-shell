use super::service::{MprisController, start_mpris_service};
use crate::shared::types::UserEvent;
use winit::event_loop::EventLoopProxy;

pub struct MprisAdapter {
    controller: MprisController,
    poster: Option<String>,
    thumbnail: Option<String>,
    logo: Option<String>,
    pub rich_metadata_active: bool,
}

impl MprisAdapter {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        let controller = start_mpris_service(proxy);
        Self {
            controller,
            poster: None,
            thumbnail: None,
            logo: None,
            rich_metadata_active: false,
        }
    }

    pub fn update_metadata(
        &mut self,
        title: Option<String>,
        artist: Option<String>,
        poster: Option<String>,
        thumbnail: Option<String>,
        logo: Option<String>,
    ) {
        self.rich_metadata_active = true;
        if let Some(p) = poster.filter(|s| !s.is_empty()) {
            self.poster = Some(p);
        }
        if let Some(t) = thumbnail.filter(|s| !s.is_empty()) {
            self.thumbnail = Some(t);
        }
        if let Some(l) = logo.filter(|s| !s.is_empty()) {
            self.logo = Some(l);
        }

        // Priority: Thumbnail > Logo > Poster (Implicitly, if we had one fallback logic)
        // User preference seems to be Thumbnail for active, Logo for idle (now removed), so we just prefer Thumbnail.

        self.controller.update_metadata(title, artist, None, None);
    }

    // Proxy methods for direct controller access if needed
    pub fn update_playback_status(&self, status: &str) {
        self.controller.update_playback_status(status);
    }

    pub fn update_position(&self, position: f64) {
        self.controller.update_position(position);
    }

    pub fn update_metadata_simple(
        &self,
        title: Option<String>,
        artist: Option<String>,
        art_url: Option<String>,
        duration: Option<f64>,
    ) {
        self.controller
            .update_metadata(title, artist, art_url, duration);
    }
}
