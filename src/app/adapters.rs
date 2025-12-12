use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::MouseScrollDelta,
    window::CursorIcon,
};

use crate::shared::types::{Cursor, MouseDelta, MousePosition, WindowSize};

impl From<MouseScrollDelta> for MouseDelta {
    fn from(value: MouseScrollDelta) -> Self {
        match value {
            MouseScrollDelta::LineDelta(x, y) => Self((x * 300.0) as f64, (y * 300.0) as f64),
            MouseScrollDelta::PixelDelta(position) => Self(position.x * 3.0, position.y * 3.0),
        }
    }
}

impl From<PhysicalSize<u32>> for WindowSize {
    fn from(value: PhysicalSize<u32>) -> Self {
        Self(value.width as i32, value.height as i32)
    }
}

impl From<PhysicalPosition<f64>> for MousePosition {
    fn from(value: PhysicalPosition<f64>) -> Self {
        Self(value.x as i32, value.y as i32)
    }
}

impl TryFrom<Cursor> for CursorIcon {
    type Error = &'static str;

    fn try_from(value: Cursor) -> Result<Self, Self::Error> {
        match value {
            Cursor::Default => Ok(Self::Default),
            Cursor::Pointer => Ok(Self::Pointer),
            Cursor::Text => Ok(Self::Text),
            Cursor::Move => Ok(Self::Move),
            Cursor::ZoomIn => Ok(Self::ZoomIn),
            Cursor::ZoomOut => Ok(Self::ZoomOut),
            Cursor::Wait => Ok(Self::Wait),
            _ => Err("Failed to convert Cursor to CursorIcon"),
        }
    }
}
