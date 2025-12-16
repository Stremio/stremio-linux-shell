use std::{ffi::CString, mem, ptr};

use epoxy::{types::*, *};

pub fn create_geometry(program: u32) -> (GLuint, GLuint) {
    unsafe {
        let vertices: [f32; 16] = [
            -1.0, -1.0, 0.0, 0.0, 1.0, -1.0, 1.0, 0.0, -1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0,
        ];

        let mut vbo = 0;
        GenBuffers(1, &mut vbo);
        BindBuffer(ARRAY_BUFFER, vbo);

        BufferData(
            ARRAY_BUFFER,
            (vertices.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
            vertices.as_ptr() as _,
            STATIC_DRAW,
        );

        let mut vao = 0;
        GenVertexArrays(1, &mut vao);
        BindVertexArray(vao);

        let pos_attrib = GetAttribLocation(program, c"position".as_ptr() as _);
        EnableVertexAttribArray(pos_attrib as GLuint);
        VertexAttribPointer(
            pos_attrib as GLuint,
            2,
            FLOAT,
            FALSE,
            (4 * mem::size_of::<GLfloat>()) as GLsizei,
            ptr::null(),
        );

        let tex_attrib = GetAttribLocation(program, c"texcoord".as_ptr() as _);
        EnableVertexAttribArray(tex_attrib as GLuint);
        VertexAttribPointer(
            tex_attrib as GLuint,
            2,
            FLOAT,
            FALSE,
            (4 * mem::size_of::<GLfloat>()) as GLsizei,
            (2 * mem::size_of::<GLfloat>()) as _,
        );

        (vao, vbo)
    }
}

pub fn compile_shader(kind: GLenum, src: &str) -> GLuint {
    unsafe {
        let shader = CreateShader(kind);
        let c_str = CString::new(src.as_bytes()).unwrap();
        ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null());
        CompileShader(shader);

        let mut success = 0;
        GetShaderiv(shader, COMPILE_STATUS, &mut success);
        if success == 0 {
            let mut len = 0;
            GetShaderiv(shader, INFO_LOG_LENGTH, &mut len);

            let mut buffer = vec![0u8; len as usize];
            GetShaderInfoLog(shader, len, ptr::null_mut(), buffer.as_mut_ptr() as *mut i8);

            panic!("Shader compile error: {}", str::from_utf8(&buffer).unwrap());
        }

        shader
    }
}

pub fn compile_vertex_shader(src: &str) -> GLuint {
    compile_shader(VERTEX_SHADER, src)
}

pub fn compile_fragment_shader(src: &str) -> GLuint {
    compile_shader(FRAGMENT_SHADER, src)
}

pub fn create_program(vertex_shader: GLuint, fragment_shader: GLuint) -> GLuint {
    unsafe {
        let program = CreateProgram();

        AttachShader(program, vertex_shader);
        AttachShader(program, fragment_shader);

        LinkProgram(program);
        UseProgram(program);

        DeleteShader(vertex_shader);
        DeleteShader(fragment_shader);

        program
    }
}

pub fn create_texture(program: GLuint, uniform_name: &str) -> (GLuint, GLint) {
    unsafe {
        let mut texture = 0;
        GenTextures(1, &mut texture);
        BindTexture(TEXTURE_2D, texture);

        TexImage2D(
            TEXTURE_2D,
            0,
            BGRA as GLint,
            1,
            1,
            0,
            BGRA,
            UNSIGNED_BYTE,
            ptr::null(),
        );

        TexParameteri(TEXTURE_2D, TEXTURE_MAX_LEVEL, 0);
        TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR as GLint);
        TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR as GLint);
        TexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, CLAMP_TO_EDGE as GLint);
        TexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, CLAMP_TO_EDGE as GLint);

        let name = CString::new(uniform_name).unwrap();
        let texture_uniform = GetUniformLocation(program, name.as_ptr() as _);

        (texture, texture_uniform)
    }
}

pub fn resize_texture(texture: GLuint, width: i32, height: i32) {
    unsafe {
        BindTexture(TEXTURE_2D, texture);
        TexImage2D(
            TEXTURE_2D,
            0,
            BGRA as GLint,
            width,
            height,
            0,
            BGRA,
            UNSIGNED_BYTE,
            ptr::null(),
        );

        BindTexture(TEXTURE_2D, 0);
    }
}

pub fn update_texture(
    texture: GLuint,
    x: GLint,
    y: GLint,
    width: GLint,
    height: GLint,
    stride: GLint,
    buffer: &[u8],
) {
    unsafe {
        BindTexture(TEXTURE_2D, texture);
        PixelStorei(UNPACK_ROW_LENGTH, stride);

        let offset = ((y * stride + x) * 4) as usize;
        let pixels = buffer.as_ptr().add(offset);

        TexSubImage2D(
            TEXTURE_2D,
            0,
            x,
            y,
            width,
            height,
            BGRA,
            UNSIGNED_BYTE,
            pixels as _,
        );

        PixelStorei(UNPACK_ROW_LENGTH, 0);
        BindTexture(TEXTURE_2D, 0);
    }
}

pub fn resize_viewport(width: i32, height: i32) {
    unsafe {
        epoxy::Viewport(0, 0, width, height);
    }
}

pub fn draw_texture(program: GLuint, texture: GLuint, texture_uniform: GLint, vao: GLuint) {
    unsafe {
        epoxy::UseProgram(program);
        epoxy::ActiveTexture(epoxy::TEXTURE0);
        epoxy::BindTexture(epoxy::TEXTURE_2D, texture);
        epoxy::Uniform1i(texture_uniform, 0);

        epoxy::BindVertexArray(vao);
        epoxy::DrawArrays(epoxy::TRIANGLE_STRIP, 0, 4);
    }
}
