use winit::event::MouseButton;

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
pub struct MouseDelta(pub i32, pub i32);

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
