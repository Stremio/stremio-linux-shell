use std::{cell::RefCell, rc::Rc};

use adw::subclass::prelude::*;
use crossbeam_queue::SegQueue;
use gtk::{
    DropTarget,
    gdk::{DragAction, FileList, MemoryFormat, MemoryTexture},
    glib::{self, Bytes, ControlFlow, Properties},
    graphene,
    prelude::*,
};

use crate::shared::{
    Frame,
    states::{KeyboardState, PointerState},
};

const BYTES_PER_PIXEL: usize = 4;

#[derive(Default, Properties)]
#[properties(wrapper_type = super::WebView)]
pub struct WebView {
    #[property(get, set)]
    dummy: std::cell::Cell<i32>,
    frame_buffer: RefCell<Vec<u8>>,
    frame_width: std::cell::Cell<i32>,
    frame_height: std::cell::Cell<i32>,
    pub pointer_state: Rc<PointerState>,
    pub keyboard_state: Rc<KeyboardState>,
    pub frames: Box<SegQueue<Frame>>,
    pub resize_callback: RefCell<Option<Box<dyn Fn(i32, i32)>>>,
}

#[glib::object_subclass]
impl ObjectSubclass for WebView {
    const NAME: &'static str = "WebView";
    type Type = super::WebView;
    type ParentType = gtk::Widget;
}

#[glib::derived_properties]
impl ObjectImpl for WebView {
    fn constructed(&self) {
        self.parent_constructed();

        let drop_target = DropTarget::new(FileList::static_type(), DragAction::COPY);
        self.obj().add_controller(drop_target);

        self.obj().add_tick_callback(|webview, _| {
            if !webview.imp().frames.is_empty() {
                webview.queue_draw();
            }

            ControlFlow::Continue
        });
    }
}

impl WidgetImpl for WebView {
    fn snapshot(&self, snapshot: &gtk::Snapshot) {
        // Apply pending frames to the CPU buffer
        let mut buffer = self.frame_buffer.borrow_mut();

        while let Some(frame) = self.frames.pop() {
            let width = self.frame_width.get();
            let height = self.frame_height.get();

            // If full dimensions changed, resize buffer
            if frame.full_width != width || frame.full_height != height {
                let new_size =
                    (frame.full_width as usize) * (frame.full_height as usize) * BYTES_PER_PIXEL;
                buffer.resize(new_size, 0);
                self.frame_width.set(frame.full_width);
                self.frame_height.set(frame.full_height);
            }

            let buf_width = self.frame_width.get() as usize;

            // Blit dirty rect into full buffer
            let src_stride = frame.width as usize * BYTES_PER_PIXEL;
            let dst_stride = buf_width * BYTES_PER_PIXEL;

            for row in 0..frame.height as usize {
                let src_offset = row * src_stride;
                let dst_offset =
                    (frame.y as usize + row) * dst_stride + frame.x as usize * BYTES_PER_PIXEL;

                if src_offset + src_stride <= frame.buffer.len()
                    && dst_offset + src_stride <= buffer.len()
                {
                    buffer[dst_offset..dst_offset + src_stride]
                        .copy_from_slice(&frame.buffer[src_offset..src_offset + src_stride]);
                }
            }
        }

        let width = self.frame_width.get();
        let height = self.frame_height.get();
        let expected_size = (width as usize) * (height as usize) * BYTES_PER_PIXEL;

        if buffer.len() != expected_size || expected_size == 0 {
            return;
        }

        let stride = width as usize * BYTES_PER_PIXEL;

        // CEF renders top-down in BGRA. GdkMemoryTexture expects top-down too.
        // MemoryFormat::B8G8R8A8_PREMULTIPLIED matches CEF's BGRA output.
        let bytes = Bytes::from(&*buffer);
        let texture = MemoryTexture::new(
            width,
            height,
            MemoryFormat::B8g8r8a8Premultiplied,
            &bytes,
            stride,
        );

        // Map texture to widget's CSS dimensions — GTK scales to device pixels
        let css_width = self.obj().width() as f32;
        let css_height = self.obj().height() as f32;

        if css_width > 0.0 && css_height > 0.0 {
            let rect = graphene::Rect::new(0.0, 0.0, css_width, css_height);
            snapshot.append_texture(&texture, &rect);
        }
    }

    fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
        self.parent_size_allocate(width, height, baseline);

        // Compute device pixel dimensions using fractional scale
        let surface = self.obj().native().and_then(|n| n.surface());
        let scale = surface.map_or(1.0, |s| s.scale());
        let dev_w = (width as f64 * scale).round() as i32;
        let dev_h = (height as f64 * scale).round() as i32;

        if let Some(callback) = self.resize_callback.borrow().as_ref() {
            callback(dev_w, dev_h);
        }
    }
}
