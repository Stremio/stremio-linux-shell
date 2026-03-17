use std::os::raw::c_int;

use crate::{
    WebViewEvent, cef_impl,
    shared::{with_gl, with_renderer_read},
    webview::SENDER,
};

cef_impl!(
    prefix = "WebView",
    name = RenderHandler,
    sys_type = cef_dll_sys::cef_render_handler_t,
    {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            with_renderer_read(|renderer| {
                if let Some(rect) = rect {
                    *rect = Rect {
                        x: 0,
                        y: 0,
                        width: renderer.width,
                        height: renderer.height,
                    };
                }
            });
        }

        // The `width` and `height` parameters may be outdated due to asynchronous updates from `on_paint` and `view_rect`.
        // We compare them against the current renderer dimensions before painting.
        // If they don't match, send a Resized event to ask for a repaint.
        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            _type_: PaintElementType,
            _dirty_rects_count: usize,
            dirty_rects: Option<&Rect>,
            buffer: *const u8,
            width: c_int,
            height: c_int,
        ) {
            with_gl(|_, _| {
                with_renderer_read(|renderer| {
                    if renderer.width == width && renderer.height == height {
                        if let Some(dirty) = dirty_rects {
                            renderer.paint(
                                dirty.x,
                                dirty.y,
                                dirty.width,
                                dirty.height,
                                buffer,
                                width,
                            );
                        } else {
                            renderer.paint(0, 0, width, height, buffer, width);
                        }

                        if let Some(sender) = SENDER.get() {
                            sender.send(WebViewEvent::Paint).ok();
                            crate::shared::wake_event_loop();
                        }
                    } else if let Some(sender) = SENDER.get() {
                        sender.send(WebViewEvent::Resized).ok();
                    }
                });
            });
        }
    }
);
