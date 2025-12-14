use std::sync::atomic::AtomicU64;
use winit::event::MouseButton;

pub static SCALE_FACTOR: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy)]
pub enum Cursor {
    Default,
    Pointer,
    Text,
    Move,
    ZoomIn,
    ZoomOut,
    Wait,
    None,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MousePosition(pub i32, pub i32);

#[derive(Debug, Default, Clone, Copy)]
pub struct MouseDelta(pub f64, pub f64);

#[derive(Debug, Clone, Copy)]
pub struct WindowSize(pub i32, pub i32);

#[derive(Debug, Clone, Copy)]
pub struct MouseState {
    pub button: MouseButton,
    pub pressed: bool,
    pub position: MousePosition,
    pub delta: MouseDelta,
    pub over: bool,
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            button: MouseButton::Left,
            pressed: Default::default(),
            position: Default::default(),
            delta: Default::default(),
            over: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MprisCommand {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    Seek(i64),
    SetPosition(i64),
    SetRate(f64),
}

pub enum UserEvent {
    Raise,
    Show,
    Hide,
    Quit,
    MpvEventAvailable,
    WebViewEventAvailable,
    MprisCommand(MprisCommand),
    MetadataUpdate {
        title: Option<String>,
        artist: Option<String>,
        poster: Option<String>,
        thumbnail: Option<String>,
        logo: Option<String>,
    },
}
