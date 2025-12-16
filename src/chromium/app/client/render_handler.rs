use std::{
    slice,
    sync::{Arc, RwLock},
};

use cef::{rc::*, *};
use flume::Sender;

use crate::{
    chromium::{ChromiumEvent, types::Viewport},
    shared::Frame,
};

wrap_render_handler! {
    pub struct ChromiumRenderHandler {
        viewport: Arc<RwLock<Viewport>>,
        sender: Sender<ChromiumEvent>,
    }

    impl RenderHandler {
        fn screen_info(&self, _browser: Option<&mut Browser>, screen_info: Option<&mut ScreenInfo>) -> i32 {
            if let Ok(viewport) = self.viewport.read()
                && let Some(screen_info) = screen_info {
                    screen_info.device_scale_factor = viewport.scale_factor as f32;

                    return true.into();
                }

            false.into()
        }

        fn screen_point(
            &self,
            _browser: Option<&mut Browser>,
            _view_x: i32,
            _view_y: i32,
            _screen_x: Option<&mut i32>,
            _screen_y: Option<&mut i32>,
        ) -> i32 {
            false.into()
        }

        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(rect) = rect
                && let Ok(viewport) = self.viewport.read() {
                    rect.width = viewport.width / viewport.scale_factor;
                    rect.height = viewport.height / viewport.scale_factor;
                }
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            _type: PaintElementType,
            dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: i32,
            height: i32,
        ) {
            if let Some(dirty_rects) = dirty_rects
                && let Some(dirty_rect) = dirty_rects.first() {
                    let size = (width * height * 4) as usize;
                    let buffer_slice = unsafe { slice::from_raw_parts(buffer, size) };
                    let buffer = Arc::from(buffer_slice);

                    let frame = Frame {
                        x: dirty_rect.x,
                        y: dirty_rect.y,
                        width: dirty_rect.width,
                        height: dirty_rect.height,
                        full_width: width,
                        full_height: height,
                        buffer,
                    };

                    self.sender.send(ChromiumEvent::Render(frame)).ok();
                }
        }
    }
}
