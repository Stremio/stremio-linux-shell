use std::{mem, ptr};

use gl::types::{GLenum, GLfloat, GLint, GLsizei, GLsizeiptr, GLuint};

use super::constants::BYTES_PER_PIXEL;

pub fn create_fbo(texture: GLuint) -> GLuint {
    unsafe {
        let mut fbo = 0;
        gl::GenFramebuffers(1, &mut fbo);
        gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
        gl::FramebufferTexture2D(
            gl::FRAMEBUFFER,
            gl::COLOR_ATTACHMENT0,
            gl::TEXTURE_2D,
            texture,
            0,
        );

        gl::BindFramebuffer(gl::FRAMEBUFFER, 0);

        fbo
    }
}

pub fn create_pbo(width: i32, height: i32) -> GLuint {
    unsafe {
        let mut pbo = 0;
        gl::GenBuffers(1, &mut pbo);

        gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, pbo);
        gl::BufferData(
            gl::PIXEL_UNPACK_BUFFER,
            (width * height * BYTES_PER_PIXEL) as GLsizeiptr,
            ptr::null(),
            gl::STREAM_DRAW,
        );

        gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, 0);

        pbo
    }
}

pub fn create_geometry(program: u32) -> (GLuint, GLuint) {
    unsafe {
        let vertices: [f32; 16] = [
            -1.0, -1.0, 0.0, 0.0, 1.0, -1.0, 1.0, 0.0, -1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0,
        ];

        let mut vbo = 0;
        gl::GenBuffers(1, &mut vbo);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

        gl::BufferData(
            gl::ARRAY_BUFFER,
            (vertices.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
            vertices.as_ptr() as _,
            gl::STATIC_DRAW,
        );

        let mut vao = 0;
        gl::GenVertexArrays(1, &mut vao);
        gl::BindVertexArray(vao);

        let pos_attrib = gl::GetAttribLocation(program, c"position".as_ptr() as _);
        gl::EnableVertexAttribArray(pos_attrib as GLuint);
        gl::VertexAttribPointer(
            pos_attrib as GLuint,
            2,
            gl::FLOAT,
            gl::FALSE,
            (4 * mem::size_of::<GLfloat>()) as GLsizei,
            ptr::null(),
        );

        let tex_attrib = gl::GetAttribLocation(program, c"texcoord".as_ptr() as _);
        gl::EnableVertexAttribArray(tex_attrib as GLuint);
        gl::VertexAttribPointer(
            tex_attrib as GLuint,
            2,
            gl::FLOAT,
            gl::FALSE,
            (4 * mem::size_of::<GLfloat>()) as GLsizei,
            (2 * mem::size_of::<GLfloat>()) as _,
        );

        (vao, vbo)
    }
}

pub fn create_texture(width: i32, height: i32, internal_format: GLuint, format: GLuint) -> GLuint {
    unsafe {
        let mut texture = 0;
        gl::GenTextures(1, &mut texture);
        gl::BindTexture(gl::TEXTURE_2D, texture);

        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            internal_format as GLint,
            width,
            height,
            0,
            format,
            gl::UNSIGNED_BYTE,
            ptr::null(),
        );

        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAX_LEVEL, 0);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(
            gl::TEXTURE_2D,
            gl::TEXTURE_WRAP_S,
            gl::CLAMP_TO_EDGE as GLint,
        );
        gl::TexParameteri(
            gl::TEXTURE_2D,
            gl::TEXTURE_WRAP_T,
            gl::CLAMP_TO_EDGE as GLint,
        );

        texture
    }
}

pub fn compile_shader(kind: GLenum, src: &str) -> GLuint {
    unsafe {
        let shader = gl::CreateShader(kind);
        let c_str = std::ffi::CString::new(src.as_bytes()).unwrap();
        gl::ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null());
        gl::CompileShader(shader);

        let mut success = 0;
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success);
        if success == 0 {
            let mut len = 0;
            gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);

            let mut buffer = vec![0u8; len as usize];
            gl::GetShaderInfoLog(shader, len, ptr::null_mut(), buffer.as_mut_ptr() as *mut i8);

            panic!(
                "Shader compile error: {}",
                std::str::from_utf8(&buffer).unwrap()
            );
        }

        shader
    }
}

pub fn resize_pbo(pbo: GLuint, width: i32, height: i32) {
    unsafe {
        gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, pbo);

        let mut pbo_size = 0;
        gl::GetBufferParameteriv(gl::PIXEL_UNPACK_BUFFER, gl::BUFFER_SIZE, &mut pbo_size);

        let new_size = width * height * BYTES_PER_PIXEL;
        if new_size > pbo_size {
            gl::BufferData(
                gl::PIXEL_UNPACK_BUFFER,
                new_size as GLsizeiptr,
                ptr::null(),
                gl::STREAM_DRAW,
            );
        }

        gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, 0);
    }
}

pub fn resize_texture(
    texture: GLuint,
    width: i32,
    height: i32,
    internal_format: GLuint,
    format: GLuint,
) {
    unsafe {
        gl::BindTexture(gl::TEXTURE_2D, texture);
        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            internal_format as GLint,
            width,
            height,
            0,
            format,
            gl::UNSIGNED_BYTE,
            ptr::null(),
        );

        gl::BindTexture(gl::TEXTURE_2D, 0);
    }
}
