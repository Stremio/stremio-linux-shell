use std::{
    cell::{OnceCell, RefCell},
    rc::Rc,
};

use gtk::glib::{self, subclass::prelude::*};
use mpris_server::{Metadata, PlaybackStatus, Player};
use tracing::error;

use crate::spawn_local;

type StatusCallback = Box<dyn Fn(bool)>;
type RaiseCallback = Box<dyn Fn()>;

#[derive(Default)]
pub struct Mpris {
    mpris: Rc<OnceCell<Player>>,
    status_callback: Rc<RefCell<Option<StatusCallback>>>,
    raise_callback: Rc<RefCell<Option<RaiseCallback>>>,
}

#[glib::object_subclass]
impl ObjectSubclass for Mpris {
    const NAME: &'static str = "Mpris";
    type Type = super::Mpris;
    type ParentType = glib::Object;
}

impl Mpris {
    pub fn start(&self, id: &'static str, name: &'static str) {
        let mpris = self.mpris.clone();
        let status_callback = self.status_callback.clone();
        let raise_callback = self.raise_callback.clone();

        spawn_local!(async move {
            let player = Player::builder(name)
                .identity(name)
                .desktop_entry(id)
                .can_play(true)
                .can_pause(true)
                .can_raise(true)
                .can_go_previous(false)
                .can_go_next(false)
                .build()
                .await
                .expect("Failed to start MPRIS server");

            if let Some(callback) = status_callback.borrow_mut().take() {
                player.connect_play_pause(move |player| {
                    let paused = matches!(player.playback_status(), PlaybackStatus::Playing);
                    callback(paused);
                });
            }

            if let Some(callback) = raise_callback.borrow_mut().take() {
                player.connect_raise(move |_| callback());
            }

            let player = mpris.get_or_init(|| player);
            player.run().await;
        });
    }

    pub fn set_status(&self, paused: bool) {
        let mpris = self.mpris.clone();

        spawn_local!(async move {
            if let Some(mpris) = mpris.get() {
                let status = match paused {
                    true => PlaybackStatus::Paused,
                    false => PlaybackStatus::Playing,
                };

                if let Err(e) = mpris.set_playback_status(status).await {
                    error!("Failed to set mpris playback status: {e}");
                }
            }
        });
    }

    pub fn set_metadata(&self, title: String, artist: Option<String>, art_url: Option<String>) {
        let mpris = self.mpris.clone();

        spawn_local!(async move {
            if let Some(mpris) = mpris.get() {
                let mut metadata = Metadata::new();
                metadata.set_title(Some(title));
                metadata.set_artist(Some(artist.map_or(vec![], |artist| vec![artist])));
                metadata.set_art_url(art_url);

                if let Err(e) = mpris.set_metadata(metadata).await {
                    error!("Failed to set mpris metadata: {e}");
                }
            }
        });
    }

    pub fn set_status_callback<F: Fn(bool) + 'static>(&self, callback: F) {
        self.status_callback
            .borrow_mut()
            .replace(Box::new(callback));
    }

    pub fn set_raise_callback<F: Fn() + 'static>(&self, callback: F) {
        self.raise_callback.borrow_mut().replace(Box::new(callback));
    }
}

impl ObjectImpl for Mpris {}
