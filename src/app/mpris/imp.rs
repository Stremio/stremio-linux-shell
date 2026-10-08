use std::{cell::OnceCell, rc::Rc, sync::OnceLock};

use gtk::glib::{self, subclass::Signal};
use gtk::{glib::clone, prelude::*, subclass::prelude::*};
use mpris_server::{Metadata, PlaybackStatus, Player};
use tracing::error;

use crate::spawn_local;

#[derive(Default)]
pub struct Mpris {
    mpris: Rc<OnceCell<Player>>,
}

#[glib::object_subclass]
impl ObjectSubclass for Mpris {
    const NAME: &'static str = "Mpris";
    type Type = super::Mpris;
    type ParentType = glib::Object;
}

impl ObjectImpl for Mpris {
    fn signals() -> &'static [Signal] {
        static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
        SIGNALS.get_or_init(|| {
            vec![
                Signal::builder("paused")
                    .param_types([bool::static_type()])
                    .build(),
                Signal::builder("raise").build(),
            ]
        })
    }
}

impl Mpris {
    pub fn start(&self, id: &'static str, name: &'static str) {
        let mpris = self.mpris.clone();
        let object = self.obj();

        let emit_paused = clone!(
            #[weak]
            object,
            move |paused: bool| {
                object.emit_by_name::<()>("paused", &[&paused]);
            }
        );

        let emit_raise = clone!(
            #[weak]
            object,
            move || {
                object.emit_by_name::<()>("raise", &[]);
            }
        );

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

            let emit = emit_paused.clone();
            player.connect_play_pause(move |player| {
                let paused = matches!(player.playback_status(), PlaybackStatus::Playing);
                emit(paused);
            });

            let emit = emit_paused.clone();
            player.connect_play(move |_| emit(false));

            let emit = emit_paused.clone();
            player.connect_pause(move |_| emit(true));

            let emit = emit_paused.clone();
            player.connect_stop(move |_| emit(true));

            player.connect_raise(move |_| emit_raise());

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
}
