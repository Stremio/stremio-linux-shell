use std::os::raw::c_int;

use crate::{
    WebViewEvent, cef_impl,
    shared::{types::UserEvent, with_gl, with_renderer_read},
    webview::{PROXY, SENDER},
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
                    // Relaxed check: Allow small rounding differences due to scaling
                    // Also explicitly allow if Paint equals Physical OR Logical (fallback)
                    let width_diff = (renderer.width - width).abs();
                    let height_diff = (renderer.height - height).abs();
                    let is_match = width_diff <= 2 && height_diff <= 2;

                    if is_match {
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
                            if let Some(proxy) = PROXY.get() {
                                proxy.send_event(UserEvent::WebViewEventAvailable).ok();
                            }
                        }
                    } else if let Some(sender) = SENDER.get() {
                        sender.send(WebViewEvent::Resized).ok();
                    }
                });
            });
        }
    }
);
