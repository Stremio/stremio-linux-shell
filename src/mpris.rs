use crate::shared::types::{MprisCommand, UserEvent};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use winit::event_loop::EventLoopProxy;
use zbus::{Connection, interface};

#[derive(Clone)]
struct MprisState {
    playback_status: String,
    loop_status: String,
    rate: f64,
    shuffle: bool,
    volume: f64,
    media_title: Option<String>,
    media_duration: Option<f64>,
    media_position: Option<f64>,
}

impl Default for MprisState {
    fn default() -> Self {
        Self {
            playback_status: "Stopped".to_string(),
            loop_status: "None".to_string(),
            rate: 1.0,
            shuffle: false,
            volume: 1.0,
            media_title: None,
            media_duration: None,
            media_position: None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum MprisStateUpdate {
    PlaybackStatus,
    Metadata,
}

pub struct MprisController {
    state: Arc<RwLock<MprisState>>,
    update_tx: UnboundedSender<MprisStateUpdate>,
}

impl MprisController {
    pub fn update_playback_status(&self, status: &str) {
        let mut state = self.state.write().unwrap();
        state.playback_status = status.to_string();
        // Send update to D-Bus thread for signal emission
        self.update_tx.send(MprisStateUpdate::PlaybackStatus).ok();
    }

    pub fn update_metadata(&self, title: Option<String>, duration: Option<f64>) {
        let mut state = self.state.write().unwrap();

        if let Some(ref t) = title {
            state.media_title = Some(t.clone());
        }

        if let Some(d) = duration {
            state.media_duration = Some(d);
        }

        if let Some(d) = duration {
            state.media_duration = Some(d);
        }

        self.update_tx.send(MprisStateUpdate::Metadata).ok();
    }

    pub fn update_position(&self, position: f64) {
        let mut state = self.state.write().unwrap();
        state.media_position = Some(position);
    }
}

fn build_metadata(state: &MprisState) -> HashMap<String, zbus::zvariant::OwnedValue> {
    let mut metadata = HashMap::new();

    metadata.insert(
        "mpris:trackid".to_string(),
        zbus::zvariant::ObjectPath::try_from("/org/mpris/MediaPlayer2/TrackList/NoTrack")
            .unwrap()
            .into(),
    );

    if let Some(ref t) = state.media_title {
        metadata.insert(
            "xesam:title".to_string(),
            zbus::zvariant::Str::from(t.clone()).into(),
        );
    }

    if let Some(d) = state.media_duration {
        let d_micros = (d * 1_000_000.0) as i64;
        metadata.insert("mpris:length".to_string(), d_micros.into());
    }

    metadata
}

// Interface: org.mpris.MediaPlayer2
struct MprisRoot {
    proxy: EventLoopProxy<UserEvent>,
    _state: Arc<RwLock<MprisState>>,
}

#[interface(name = "org.mpris.MediaPlayer2")]
impl MprisRoot {
    fn raise(&self) {
        self.proxy.send_event(UserEvent::Raise).ok();
    }

    fn quit(&self) {
        self.proxy.send_event(UserEvent::Quit).ok();
    }

    #[zbus(property)]
    fn can_quit(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_raise(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn identity(&self) -> String {
        "Stremio".to_string()
    }

    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        vec!["stremio".to_string()]
    }

    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> {
        vec![]
    }
}

// Interface: org.mpris.MediaPlayer2.Player
struct MprisPlayerImpl {
    proxy: EventLoopProxy<UserEvent>,
    state: Arc<RwLock<MprisState>>,
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl MprisPlayerImpl {
    fn next(&self) {
        self.proxy
            .send_event(UserEvent::MprisCommand(MprisCommand::Next))
            .ok();
    }

    fn previous(&self) {
        self.proxy
            .send_event(UserEvent::MprisCommand(MprisCommand::Previous))
            .ok();
    }

    fn pause(&self) {
        self.proxy
            .send_event(UserEvent::MprisCommand(MprisCommand::Pause))
            .ok();
    }

    fn play_pause(&self) {
        self.proxy
            .send_event(UserEvent::MprisCommand(MprisCommand::PlayPause))
            .ok();
    }

    fn stop(&self) {
        self.proxy
            .send_event(UserEvent::MprisCommand(MprisCommand::Stop))
            .ok();
    }

    fn play(&self) {
        self.proxy
            .send_event(UserEvent::MprisCommand(MprisCommand::Play))
            .ok();
    }

    fn seek(&self, offset: i64) {
        self.proxy
            .send_event(UserEvent::MprisCommand(MprisCommand::Seek(offset)))
            .ok();
    }

    fn set_position(&self, _track_id: zbus::zvariant::ObjectPath<'_>, position: i64) {
        self.proxy
            .send_event(UserEvent::MprisCommand(MprisCommand::SetPosition(position)))
            .ok();
    }

    fn open_uri(&self, _uri: String) {
        // Not implemented
    }

    #[zbus(property)]
    fn playback_status(&self) -> String {
        self.state.read().unwrap().playback_status.clone()
    }

    #[zbus(property)]
    fn loop_status(&self) -> String {
        self.state.read().unwrap().loop_status.clone()
    }

    #[zbus(property)]
    fn set_loop_status(&self, loop_status: String) {
        self.state.write().unwrap().loop_status = loop_status;
    }

    #[zbus(property)]
    fn rate(&self) -> f64 {
        self.state.read().unwrap().rate
    }

    #[zbus(property)]
    fn set_rate(&self, rate: f64) {
        self.state.write().unwrap().rate = rate;
        self.proxy
            .send_event(UserEvent::MprisCommand(MprisCommand::SetRate(rate)))
            .ok();
    }

    #[zbus(property)]
    fn shuffle(&self) -> bool {
        self.state.read().unwrap().shuffle
    }

    #[zbus(property)]
    fn set_shuffle(&self, shuffle: bool) {
        self.state.write().unwrap().shuffle = shuffle;
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, zbus::zvariant::OwnedValue> {
        build_metadata(&self.state.read().unwrap())
    }

    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.state.read().unwrap().volume
    }

    #[zbus(property)]
    fn set_volume(&self, volume: f64) {
        self.state.write().unwrap().volume = volume;
    }

    #[zbus(property)]
    fn position(&self) -> i64 {
        if let Some(position) = self.state.read().unwrap().media_position {
            (position * 1_000_000.0) as i64
        } else {
            0
        }
    }

    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        0.1
    }

    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        8.0
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }
}

pub fn start_mpris_service(proxy: EventLoopProxy<UserEvent>) -> MprisController {
    let state = Arc::new(RwLock::new(MprisState::default()));
    let state_clone = state.clone();
    let (update_tx, mut update_rx) = unbounded_channel::<MprisStateUpdate>();

    std::thread::spawn(move || {
        let runtime = Runtime::new().unwrap();
        runtime.block_on(async move {
            let root = MprisRoot {
                proxy: proxy.clone(),
                _state: state_clone.clone(),
            };

            let player = MprisPlayerImpl {
                proxy,
                state: state_clone.clone(),
            };

            let conn = Connection::session()
                .await
                .expect("Failed to connect to session bus");

            conn.request_name("org.mpris.MediaPlayer2.stremio")
                .await
                .expect("Failed to request name");

            let object_server = conn.object_server();
            object_server
                .at("/org/mpris/MediaPlayer2", root)
                .await
                .expect("Failed to serve root interface");

            object_server
                .at("/org/mpris/MediaPlayer2", player)
                .await
                .expect("Failed to serve player interface");

            // Process state updates and emit property change signals
            while let Some(update) = update_rx.recv().await {
                match update {
                    MprisStateUpdate::PlaybackStatus => {
                        // Get interface reference and emit signal
                        if let Ok(iface_ref) = object_server
                            .interface::<_, MprisPlayerImpl>("/org/mpris/MediaPlayer2")
                            .await
                        {
                            let ctxt = iface_ref.signal_context();
                            iface_ref
                                .get()
                                .await
                                .playback_status_changed(ctxt)
                                .await
                                .ok();
                        }
                    }
                    MprisStateUpdate::Metadata => {
                        if let Ok(iface_ref) = object_server
                            .interface::<_, MprisPlayerImpl>("/org/mpris/MediaPlayer2")
                            .await
                        {
                            let ctxt = iface_ref.signal_context();
                            iface_ref.get().await.metadata_changed(ctxt).await.ok();
                        }
                    }
                }
            }
        });
    });

    MprisController { state, update_tx }
}
