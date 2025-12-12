mod constants;
mod utils;

use std::ptr;

use constants::{BYTES_PER_PIXEL, FRAGMENT_SRC, VERTEX_SRC};
use gl::types::{GLint, GLuint};

#[derive(Debug)]
pub struct Renderer {
    pub program: GLuint,
    pub front_texture: GLuint,
    pub front_uniform: GLint,
    pub back_texture: GLuint,
    pub back_uniform: GLint,
    pub vao: GLuint,
    pub vbo: GLuint,
    pub fbo: GLuint,
    pub pbos: [GLuint; 2],
    pub pbo_index: std::sync::atomic::AtomicUsize,
    pub width: i32,
    pub height: i32,
    pub refresh_rate: u32,
    pub renderer_name: String,
}

// Helper structs for parallel PBO upload

impl Renderer {
    pub fn new((width, height): (i32, i32), refresh_rate: u32) -> Self {
        unsafe {
            let vertex_shader = utils::compile_shader(gl::VERTEX_SHADER, VERTEX_SRC);
            let fragment_shader = utils::compile_shader(gl::FRAGMENT_SHADER, FRAGMENT_SRC);
            let program = gl::CreateProgram();

            gl::AttachShader(program, vertex_shader);
            gl::AttachShader(program, fragment_shader);

            gl::LinkProgram(program);
            gl::UseProgram(program);

            gl::DeleteShader(vertex_shader);
            gl::DeleteShader(fragment_shader);

            let front_texture = utils::create_texture(width, height);
            let front_uniform = gl::GetUniformLocation(program, c"front_texture".as_ptr() as _);

            let back_texture = utils::create_texture(width, height);
            let back_uniform = gl::GetUniformLocation(program, c"back_texture".as_ptr() as _);

            let (vao, vbo) = utils::create_geometry(program);
            let fbo = utils::create_fbo(back_texture);

            let pbo1 = utils::create_pbo(width, height);
            let pbo2 = utils::create_pbo(width, height);

            let status = gl::CheckFramebufferStatus(gl::FRAMEBUFFER);
            if status != gl::FRAMEBUFFER_COMPLETE {
                panic!("Framebuffer not complete: {status}");
            }

            let renderer_name = std::ffi::CStr::from_ptr(gl::GetString(gl::RENDERER) as *const i8)
                .to_string_lossy()
                .into_owned();

            Self {
                program,
                front_texture,
                front_uniform,
                back_texture,
                back_uniform,
                vao,
                vbo,
                fbo,
                pbos: [pbo1, pbo2],
                pbo_index: std::sync::atomic::AtomicUsize::new(0),
                width,
                height,
                refresh_rate,
                renderer_name,
            }
        }
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        unsafe {
            self.width = width;
            self.height = height;

            gl::Viewport(0, 0, width, height);

            gl::Viewport(0, 0, width, height);

            utils::resize_pbo(self.pbos[0], width, height);
            utils::resize_pbo(self.pbos[1], width, height);
            utils::resize_texture(self.back_texture, width, height);
            utils::resize_texture(self.front_texture, width, height);
        }
    }

    // A Pixel Buffer Object (PBO) is used to upload the buffer directly to the GPU,
    // offering better performance than direct texture uploads.
    // This helps reduce the time the current GL context remains locked.
    pub fn paint(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        buffer: *const u8,
        full_width: i32,
    ) {
        unsafe {
            // Swap PBO index (Double Buffering)
            let current = self.pbo_index.load(std::sync::atomic::Ordering::Relaxed);
            let next_index = (current + 1) % 2;
            self.pbo_index
                .store(next_index, std::sync::atomic::Ordering::Relaxed);
            let next_pbo = self.pbos[next_index];

            gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, next_pbo);

            // No glBufferData call here! We reuse the pre-allocated buffer.
            // This eliminates the 5ms allocation stall.

            let row_bytes = width * BYTES_PER_PIXEL;
            let stride = full_width * BYTES_PER_PIXEL;

            let ptr = gl::MapBuffer(gl::PIXEL_UNPACK_BUFFER, gl::WRITE_ONLY) as *mut u8;

            if !ptr.is_null() {
                use rayon::prelude::*;

                // Cast to usize to allow sending to threads
                let dst_addr = ptr as usize;
                let src_addr = buffer as usize;

                let total_bytes = width * height * BYTES_PER_PIXEL;
                let use_parallel = total_bytes > 4_194_304; // 4MB threshold

                if use_parallel {
                    // Calculate optimal chunk size to avoid Rayon overhead
                    // Target ~32KB per task
                    let bytes_per_row = (width * BYTES_PER_PIXEL) as usize;
                    let min_rows = std::cmp::max(1, 32_768 / bytes_per_row);

                    if width == full_width {
                        (0..height)
                            .into_par_iter()
                            .with_min_len(min_rows)
                            .for_each(move |row| {
                                let row_len = stride as usize;
                                let src_offset = (y * stride + x * BYTES_PER_PIXEL) as usize
                                    + (row as usize * row_len);
                                let dst_offset = row as usize * row_len;

                                let src = (src_addr as *const u8).add(src_offset);
                                let dst = (dst_addr as *mut u8).add(dst_offset);

                                ptr::copy_nonoverlapping(src, dst, row_len);
                            });
                    } else {
                        (0..height)
                            .into_par_iter()
                            .with_min_len(min_rows)
                            .for_each(move |row| {
                                let src_offset = (y + row) * stride + (x * BYTES_PER_PIXEL);
                                let dst_offset = row * row_bytes;

                                let src = (src_addr as *const u8).add(src_offset as usize);
                                let dst = (dst_addr as *mut u8).add(dst_offset as usize);

                                ptr::copy_nonoverlapping(src, dst, row_bytes as usize);
                            });
                    }
                } else {
                    // Sequential copy for small updates to avoid thread pool overhead
                    if width == full_width {
                        (0..height).for_each(move |row| {
                            let row_len = stride as usize;
                            let src_offset = (y * stride + x * BYTES_PER_PIXEL) as usize
                                + (row as usize * row_len);
                            let dst_offset = row as usize * row_len;

                            let src = (src_addr as *const u8).add(src_offset);
                            let dst = (dst_addr as *mut u8).add(dst_offset);

                            ptr::copy_nonoverlapping(src, dst, row_len);
                        });
                    } else {
                        (0..height).for_each(move |row| {
                            let src_offset = (y + row) * stride + (x * BYTES_PER_PIXEL);
                            let dst_offset = row * row_bytes;

                            let src = (src_addr as *const u8).add(src_offset as usize);
                            let dst = (dst_addr as *mut u8).add(dst_offset as usize);

                            ptr::copy_nonoverlapping(src, dst, row_bytes as usize);
                        });
                    }
                }

                gl::UnmapBuffer(gl::PIXEL_UNPACK_BUFFER);
            }

            gl::BindTexture(gl::TEXTURE_2D, self.front_texture);
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                x,
                y,
                width,
                height,
                gl::BGRA,
                gl::UNSIGNED_BYTE,
                std::ptr::null(),
            );

            gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, 0);
        }
    }

    pub fn draw(&self) {
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
            gl::BlendEquation(gl::FUNC_ADD);

            gl::UseProgram(self.program);

            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.back_texture);
            gl::Uniform1i(self.back_uniform, 0);

            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D, self.front_texture);
            gl::Uniform1i(self.front_uniform, 1);

            gl::BindVertexArray(self.vao);
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.program);
            gl::DeleteTextures(1, &self.front_texture);
            gl::DeleteTextures(1, &self.back_texture);
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteVertexArrays(1, &self.vao);
            gl::DeleteBuffers(2, self.pbos.as_ptr());
            gl::DeleteBuffers(1, &self.fbo);
        }
    }
}
