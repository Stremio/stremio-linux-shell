use std::cell::RefCell;
use std::{rc::Rc, sync::OnceLock};

use gtk::glib::JoinHandle;
use gtk::glib::{self, subclass::Signal};
use gtk::{glib::clone, prelude::*, subclass::prelude::*};
use mpris_server::{Metadata, PlaybackStatus, Player};
use tracing::error;

use crate::app::config::{APP_ID, APP_NAME};
use crate::spawn_local;

#[derive(Default)]
pub struct Mpris {
    player: RefCell<Option<Rc<Player>>>,
    task: RefCell<Option<JoinHandle<()>>>,
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
    pub fn start(&self) {
        self.stop();

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

        let task = spawn_local!(clone!(
            #[weak]
            object,
            async move {
                let player = Player::builder(APP_NAME)
                    .identity(APP_NAME)
                    .desktop_entry(APP_ID)
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

                let player = Rc::new(player);
                object.imp().player.replace(Some(player.clone()));

                player.run().await;
            }
        ));

        self.task.replace(Some(task));
    }

    pub fn stop(&self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }

        self.player.replace(None);
    }

    pub fn set_status(&self, paused: bool) {
        if let Some(player) = self.player.borrow().clone() {
            spawn_local!(async move {
                let status = match paused {
                    true => PlaybackStatus::Paused,
                    false => PlaybackStatus::Playing,
                };

                if let Err(e) = player.set_playback_status(status).await {
                    error!("Failed to set playback status: {e}");
                }
            });
        }
    }

    pub fn set_metadata(&self, title: String, artist: Option<String>, art_url: Option<String>) {
        if let Some(player) = self.player.borrow().clone() {
            spawn_local!(async move {
                let mut metadata = Metadata::new();
                metadata.set_title(Some(title));
                metadata.set_artist(Some(artist.map_or(vec![], |artist| vec![artist])));
                metadata.set_art_url(art_url);

                if let Err(e) = player.set_metadata(metadata).await {
                    error!("Failed to set metadata: {e}");
                }
            });
        }
    }
}
