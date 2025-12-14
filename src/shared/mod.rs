pub mod ipc;
pub mod states;

use std::sync::Arc;

#[derive(Default, Debug)]
pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub full_width: i32,
    pub full_height: i32,
    pub buffer: Arc<[u8]>,
}
