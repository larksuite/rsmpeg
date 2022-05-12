//! A program that renders and saves GLSL output to a video file. The render
//! output looks like the shadertoy, but it's rendered frame by frame.

use anyhow::{bail, Context as AnyhowContext, Result};
use cstr::cstr;
use gl::types::*;
use glfw::{Context, OpenGlProfileHint, Window, WindowHint};
use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext},
    avformat::AVFormatContextOutput,
    avutil::{ra, AVFrame},
    error::RsmpegError,
};
use std::{
    ffi::{CStr, CString},
    fs,
    mem::size_of,
    ptr::null,
};

static VERTICES: [f32; 8] = [-1., -1., 1., -1., 1., 1., -1., 1.];
static INDICES: [GLuint; 6] = [0, 1, 2, 2, 3, 0];

/// This function creates a shader object for you.
fn create_shader(shader_type: GLenum, shader_source: &str) -> Result<GLuint> {
    unsafe {
        let shader = gl::CreateShader(shader_type);
        let shader_source = CString::new(shader_source)?;
        gl::ShaderSource(shader, 1, &shader_source.as_ptr() as _, null());
        gl::CompileShader(shader);
        let mut compile_status = 0i32;
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut compile_status as _);
        if compile_status != gl::TRUE as _ {
            let mut buf = [0i8; 512];
            gl::GetShaderInfoLog(shader, 512, &mut 0, buf.as_mut_ptr());
            bail!(std::ffi::CStr::from_ptr(buf.as_ptr()).to_string_lossy());
        }
        Ok(shader)
    }
}

/// This function creates a program object for you.
fn create_program(vertex_shader: &str, fragment_shader: &str) -> Result<GLuint> {
    unsafe {
        // TODO program vertex_shader fragment_shader freeing on error.
        let program = gl::CreateProgram();
        let vertex_shader = create_shader(gl::VERTEX_SHADER, vertex_shader)?;
        let fragment_shader = create_shader(gl::FRAGMENT_SHADER, fragment_shader)?;
        gl::AttachShader(program, vertex_shader);
        gl::AttachShader(program, fragment_shader);
        gl::LinkProgram(program);
        gl::ValidateProgram(program);

        let mut validate_status = 0;
        gl::GetProgramiv(program, gl::VALIDATE_STATUS, &mut validate_status as _);
        if validate_status != gl::TRUE as _ {
            gl::DeleteShader(vertex_shader);
            gl::DeleteShader(fragment_shader);
            gl::DeleteProgram(program);

            let mut buf = [0i8; 512];
            gl::GetProgramInfoLog(program, 512, &mut 0, buf.as_mut_ptr());
            bail!(std::ffi::CStr::from_ptr(buf.as_ptr()).to_string_lossy());
        }

        gl::DeleteShader(vertex_shader);
        gl::DeleteShader(fragment_shader);
        Ok(program)
    }
}

/// Return the size of the frame buffer in glfw window.(it doesn't equal to
/// window size on Mac). It's needed because:
/// https://stackoverflow.com/questions/35715579/opengl-created-window-size-twice-as-large
fn frame_buffer_size(window: &Window) -> (i32, i32) {
    let mut width = 0i32;
    let mut height = 0i32;
    unsafe {
        glfw::ffi::glfwGetFramebufferSize(window.window_ptr(), &mut width as _, &mut height as _)
    };
    (width, height)
}

/// Get data of screen buffer.
fn screen_buffer(width: i32, height: i32) -> Vec<u8> {
    let mut buffer = vec![0u8; (width * height * 3) as _];
    let buffer_ptr = buffer.as_mut_ptr() as _;
    unsafe { gl::ReadPixels(0, 0, width, height, gl::RGB, gl::UNSIGNED_BYTE, buffer_ptr) };
    buffer
}

/// Put given frame into `encode_context` and extract output packets to
/// `output_format_context`.
fn encode(
    encode_context: &mut AVCodecContext,
    frame: Option<&AVFrame>,
    output_format_context: &mut AVFormatContextOutput,
) -> Result<()> {
    encode_context.send_frame(frame)?;
    loop {
        let mut packet = match encode_context.receive_packet() {
            Ok(packet) => packet,
            Err(RsmpegError::EncoderDrainError) | Err(RsmpegError::EncoderFlushedError) => break,
            Err(e) => return Err(e.into()),
        };
        // We should rescale timestamp to sync with `output_format_context`
        // because `output_format_context`'s time_base will be change after
        // write_header.
        // https://lists.ffmpeg.org/pipermail/libav-user/2018-January/010843.html
        packet.rescale_ts(
            encode_context.time_base,
            output_format_context.streams().get(0).unwrap().time_base,
        );
        output_format_context.write_frame(&mut packet)?;
    }
    Ok(())
}

fn shadertoy(
    vertex_shader_path: &str,
    fragment_shader_path: &str,
    output_video_path: &CStr,
) -> Result<()> {
    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS)?;

    // The targeted version of macOS only supports forward-compatible core
    // profile contexts for OpenGL 3.2 and above.
    glfw.window_hint(WindowHint::ContextVersionMajor(3));
    glfw.window_hint(WindowHint::ContextVersionMinor(3));
    glfw.window_hint(WindowHint::OpenGlForwardCompat(true));
    glfw.window_hint(WindowHint::OpenGlProfile(OpenGlProfileHint::Core));

    let (mut window, _) = glfw
        .create_window(800, 600, "main_window", glfw::WindowMode::Windowed)
        .context("Create window failed")?;

    // Load OpenGL symbol.
    gl::load_with(|symbol| window.get_proc_address(symbol) as _);

    window.make_current();

    // Get width and height of the frame buffer.
    let (width, height) = frame_buffer_size(&window);

    // Init VAO
    let mut vertex_array = 0u32;
    unsafe {
        gl::GenVertexArrays(1, &mut vertex_array as _);
        gl::BindVertexArray(vertex_array);
    }

    // Init vertex buffer
    let mut vertex_buffer = 0u32;
    unsafe {
        gl::GenBuffers(1, &mut vertex_buffer as _);
        gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            size_of::<[f32; 8]>() as _,
            VERTICES.as_ptr() as _,
            gl::STATIC_DRAW,
        );
    }

    // Init index buffer
    let mut index_buffer = 0u32;
    unsafe {
        gl::GenBuffers(1, &mut index_buffer as _);
        gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, index_buffer);
        gl::BufferData(
            gl::ELEMENT_ARRAY_BUFFER,
            size_of::<[GLuint; 6]>() as _,
            INDICES.as_ptr() as _,
            gl::STATIC_DRAW,
        );
    }

    // Init vertex attrib array
    unsafe {
        gl::EnableVertexAttribArray(0);
        gl::VertexAttribPointer(
            0,
            2,
            gl::FLOAT,
            gl::FALSE,
            size_of::<[f32; 2]>() as _,
            null(),
        );
    }

    // Create opengl program from vertex shader and fragment shader.
    let vertex_shader = std::fs::read_to_string(vertex_shader_path)?;
    let fragment_shader = std::fs::read_to_string(fragment_shader_path)?;
    let program = create_program(&vertex_shader, &fragment_shader)?;
    unsafe { gl::UseProgram(program) };

    // Retrieve the `iTime` position in program for future value assigning.
    let itime = cstr!("iTime");
    let location_time = unsafe { gl::GetUniformLocation(program, itime.as_ptr()) };
    if location_time < 0 {
        bail!("cannot find location of iTime");
    }

    // Retrieve the `iResolution` position in program for future value assigning.
    let iresolution = cstr!("iResolution");
    let location_resolution = unsafe { gl::GetUniformLocation(program, iresolution.as_ptr()) };
    if location_resolution < 0 {
        bail!("cannot find location of iResolution");
    }

    // Set current frame width and height to `iResolution`.
    unsafe { gl::Uniform2i(location_resolution, width, height) }

    // Get a encode_context for frame encoding.
    let mut encode_context = {
        let encoder =
            AVCodec::find_encoder_by_name(cstr!("png")).context("Failed to find encoder codec")?;
        let mut encode_context = AVCodecContext::new(&encoder);
        encode_context.set_bit_rate(400000);
        encode_context.set_width(width);
        encode_context.set_height(height);
        encode_context.set_time_base(ra(1, 60));
        encode_context.set_framerate(ra(60, 1));
        encode_context.set_gop_size(10);
        encode_context.set_max_b_frames(1);
        encode_context.set_pix_fmt(rsmpeg::ffi::AVPixelFormat_AV_PIX_FMT_RGB24);
        encode_context.open(None)?;
        encode_context
    };

    // Create a reusable frame buffer holder.
    let mut frame = AVFrame::new();
    frame.set_format(encode_context.pix_fmt);
    frame.set_width(encode_context.width);
    frame.set_height(encode_context.height);
    frame.alloc_buffer()?;

    // Create a output_format_context for video container writing.
    let mut output_format_context = {
        let mut output_format_context = AVFormatContextOutput::create(output_video_path, None)?;
        {
            let mut stream = output_format_context.new_stream();
            stream.set_codecpar(encode_context.extract_codecpar());
            stream.set_time_base(encode_context.time_base);
        }
        output_format_context.dump(0, output_video_path)?;
        output_format_context.write_header(&mut None)?;
        output_format_context
    };

    let mut i = 0;
    while !window.should_close() {
        unsafe {
            // Render video in 45fps, play in 60fps
            gl::Uniform1f(location_time, i as f32 / 45.);
            gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, null());
        }
        let buffer = screen_buffer(width, height);

        let data = frame.data[0];
        let linesize = frame.linesize[0] as usize;
        let width = width as usize;
        let height = height as usize;
        let rgb_data = unsafe { std::slice::from_raw_parts_mut(data, height * linesize * 3) };
        for y in 0..height {
            for x in 0..width {
                rgb_data[y * linesize + x * 3 + 0] = buffer[(y * width + x) * 3 + 0];
                rgb_data[y * linesize + x * 3 + 1] = buffer[(y * width + x) * 3 + 1];
                rgb_data[y * linesize + x * 3 + 2] = buffer[(y * width + x) * 3 + 2];
            }
        }

        // Set pts for frame, `encode_context` will help us creating dts.
        frame.set_pts(i);

        // Write frame
        encode(
            &mut encode_context,
            Some(&frame),
            &mut output_format_context,
        )?;

        window.swap_buffers();
        glfw.poll_events();

        i += 1;
    }

    // Flush the encode_context.
    encode(&mut encode_context, None, &mut output_format_context)?;

    output_format_context.write_trailer()?;

    unsafe { gl::DeleteProgram(program) };
    unsafe { gl::DeleteBuffers(1, &index_buffer as _) };
    unsafe { gl::DeleteBuffers(1, &vertex_buffer as _) };
    unsafe { gl::DeleteVertexArrays(1, &vertex_array as _) };

    Ok(())
}

fn main() {
    fs::create_dir_all("./tests/output/shadertoy").unwrap();
    shadertoy(
        "./tests/assets/shaders/vt_shader.glsl",
        "./tests/assets/shaders/fg_shader_tunnel.glsl",
        cstr!("./tests/output/shadertoy/tunnel.mov"),
    )
    .unwrap();
}
