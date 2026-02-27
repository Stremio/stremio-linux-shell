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

const BYTES_PER_PIXEL: usize = 4;

wrap_render_handler! {
    pub struct ChromiumRenderHandler {
        viewport: Arc<RwLock<Viewport>>,
        sender: Sender<ChromiumEvent>,
    }

    impl RenderHandler {
        fn screen_info(&self, _browser: Option<&mut Browser>, screen_info: Option<&mut ScreenInfo>) -> i32 {
            if let Some(screen_info) = screen_info {
                // CEF OSR doesn't apply device_scale_factor to the paint buffer,
                // so we set it to 1.0 and give device pixel dimensions directly
                // as view_rect. DPI scaling is handled via zoom level instead.
                screen_info.device_scale_factor = 1.0;
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
                    // Return device pixel dimensions directly since
                    // device_scale_factor is 1.0
                    rect.width = viewport.width;
                    rect.height = viewport.height;
                    tracing::debug!(
                        "view_rect: dev={}x{} scale={}",
                        viewport.width, viewport.height, viewport.scale_factor
                    );
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
            tracing::debug!("on_paint: buffer={}x{}", width, height);
            if let Some(dirty_rects) = dirty_rects {
                for dirty_rect in dirty_rects {
                    let x = dirty_rect.x as usize;
                    let y = dirty_rect.y as usize;
                    let dirty_width = dirty_rect.width as usize;
                    let dirty_height = dirty_rect.height as usize;

                    let mut dirty_buffer = Vec::with_capacity(dirty_width * dirty_height * BYTES_PER_PIXEL);
                    let stride = (width as usize) * BYTES_PER_PIXEL;

                    unsafe {
                        for row in y..(y + dirty_height) {
                            let offset = row * stride + x * BYTES_PER_PIXEL;
                            let row_data = slice::from_raw_parts(
                                buffer.add(offset),
                                dirty_width * BYTES_PER_PIXEL,
                            );
                            dirty_buffer.extend_from_slice(row_data);
                        }
                    }

                    let frame = Frame {
                        x: dirty_rect.x,
                        y: dirty_rect.y,
                        width: dirty_rect.width,
                        height: dirty_rect.height,
                        full_width: width,
                        full_height: height,
                        buffer: Arc::from(dirty_buffer),
                    };

                    self.sender.send(ChromiumEvent::Render(frame)).ok();
                }
            }
        }
    }
}
