use std::{cell::Cell, rc::Rc};

use adw::subclass::prelude::*;
use crossbeam_queue::SegQueue;
use epoxy::types::{GLint, GLuint};
use gtk::{
    DropTarget,
    gdk::{DragAction, FileList, GLContext},
    glib::{self, ControlFlow, Propagation, Properties},
    prelude::*,
};

use crate::{
    app::webview::gl,
    shared::{
        Frame,
        states::{KeyboardState, PointerState},
    },
};

pub const FRAGMENT_SRC: &str = include_str!("shader.frag");
pub const VERTEX_SRC: &str = include_str!("shader.vert");
pub const UPDATES_PER_RENDER: i32 = 8;

#[derive(Default, Properties)]
#[properties(wrapper_type = super::WebView)]
pub struct WebView {
    #[property(get, set)]
    scale_factor: Cell<i32>,
    program: Cell<GLuint>,
    vao: Cell<GLuint>,
    vbo: Cell<GLuint>,
    texture: Cell<GLuint>,
    texture_uniform: Cell<GLint>,
    pub pointer_state: Rc<PointerState>,
    pub keyboard_state: Rc<KeyboardState>,
    pub frames: Box<SegQueue<Frame>>,
}

#[glib::object_subclass]
impl ObjectSubclass for WebView {
    const NAME: &'static str = "WebView";
    type Type = super::WebView;
    type ParentType = gtk::GLArea;
}

#[glib::derived_properties]
impl ObjectImpl for WebView {
    fn constructed(&self) {
        self.parent_constructed();

        let drop_target = DropTarget::new(FileList::static_type(), DragAction::COPY);
        self.obj().add_controller(drop_target);
    }
}

impl WidgetImpl for WebView {
    fn realize(&self) {
        self.parent_realize();

        let gl_area = self.obj();
        gl_area.make_current();

        if gl_area.error().is_some() {
            return;
        }

        let vertex_shader = gl::compile_vertex_shader(VERTEX_SRC);
        let fragment_shader = gl::compile_fragment_shader(FRAGMENT_SRC);
        let program = gl::create_program(vertex_shader, fragment_shader);
        let (vao, vbo) = gl::create_geometry(program);
        let (texture, texture_uniform) = gl::create_texture(program, "text_uniform");

        self.program.set(program);
        self.vao.set(vao);
        self.vbo.set(vbo);
        self.texture.set(texture);
        self.texture_uniform.set(texture_uniform);

        self.obj().add_tick_callback(|webview, _| {
            if !webview.imp().frames.is_empty() {
                webview.queue_render();
            }

            ControlFlow::Continue
        });
    }

    fn unrealize(&self) {
        unsafe {
            epoxy::DeleteProgram(self.program.get());
            epoxy::DeleteTextures(1, &self.texture.get());
            epoxy::DeleteBuffers(1, &self.vbo.get());
            epoxy::DeleteVertexArrays(1, &self.vao.get());
        }

        self.program.take();
        self.vao.take();
        self.vbo.take();
        self.texture.take();
        self.texture_uniform.take();

        self.parent_unrealize();
    }
}

impl GLAreaImpl for WebView {
    fn render(&self, _: &GLContext) -> Propagation {
        let scale_factor = self.scale_factor.get();

        for _ in 0..UPDATES_PER_RENDER {
            if let Some(frame) = self.frames.pop() {
                gl::resize_texture(self.texture.get(), frame.full_width, frame.full_height);
                gl::update_texture(
                    self.texture.get(),
                    frame.x,
                    frame.y,
                    frame.width,
                    frame.height,
                    frame.full_width,
                    &frame.buffer,
                );

                unsafe {
                    epoxy::Viewport(
                        0,
                        0,
                        frame.full_width * scale_factor,
                        frame.full_height * scale_factor,
                    );

                    epoxy::UseProgram(self.program.get());
                    epoxy::ActiveTexture(epoxy::TEXTURE0);
                    epoxy::BindTexture(epoxy::TEXTURE_2D, self.texture.get());
                    epoxy::Uniform1i(self.texture_uniform.get(), 0);

                    epoxy::BindVertexArray(self.vao.get());
                    epoxy::DrawArrays(epoxy::TRIANGLE_STRIP, 0, 4);
                }
            } else {
                break;
            }
        }

        Propagation::Proceed
    }
}
