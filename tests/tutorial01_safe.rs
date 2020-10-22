// http://dranger.com/ffmpeg/tutorial01.c
use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext},
    avformat::AVFormatContextInput,
    avutil::{AVFrame, AVFrameWithImageBuffer, AVImage},
    error::*,
    ffi,
    swscale::SwsContext,
};
use std::{
    ffi::{CStr, CString},
    fs::File,
    io::prelude::*,
    slice,
};

macro_rules! cstr {
    ($s: literal) => {
        &CString::new($s).unwrap()
    };
    ($s: expr) => {
        &CString::new($s)?
    };
}

fn save_frame(out_folder: &str, frame: &AVFrame, width: i32, height: i32, frame_index: i32) {
    let data = frame.data[0];
    let linesize = frame.linesize[0] as usize;

    let width = width as usize;
    let height = height as usize;

    let file_path = format!("{}/frame{}.ppm", out_folder, frame_index);
    let mut file = File::create(file_path).unwrap();
    file.write_all(&format!("P6\n{} {}\n255\n", width, height).into_bytes())
        .unwrap();
    for y in 0..height {
        let buffer = unsafe { slice::from_raw_parts(data.add(y * linesize), width * 3) };
        file.write_all(buffer).unwrap()
    }
}

#[allow(deprecated)]
fn _main(file: &CStr, out_folder: &str) -> Result<()> {
    let mut input_format_context = AVFormatContextInput::open(file)?;
    input_format_context.dump(0, file)?;
    let video_stream_index = input_format_context
        .streams()
        .into_iter()
        .position(|stream| stream.codecpar().codec_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO)
        .unwrap();
    let mut decode_context = {
        let video_stream = input_format_context
            .streams()
            .get(video_stream_index)
            .unwrap();
        let decoder = AVCodec::find_decoder(video_stream.codecpar().codec_id).unwrap();
        let mut decode_context = AVCodecContext::new(&decoder);
        decode_context.set_codecpar(video_stream.codecpar())?;
        decode_context.open(None)?;
        decode_context
    };

    let mut image_buffer = AVImage::new(
        ffi::AVPixelFormat_AV_PIX_FMT_RGB24,
        decode_context.width,
        decode_context.height,
        1,
    )
    .unwrap();
    let mut frame_rgb = AVFrameWithImageBuffer::new(
        &mut image_buffer,
        decode_context.width,
        decode_context.height,
        ffi::AVPixelFormat_AV_PIX_FMT_RGB24,
    );

    let mut sws_context = SwsContext::get_context(
        decode_context.width,
        decode_context.height,
        decode_context.pix_fmt,
        decode_context.width,
        decode_context.height,
        ffi::AVPixelFormat_AV_PIX_FMT_RGB24,
        ffi::SWS_BILINEAR,
    )
    .unwrap();

    let mut i = 0;
    while let Some(packet) = input_format_context.read_packet().unwrap() {
        if packet.stream_index == video_stream_index as i32 {
            let frame = decode_context.decode_packet(&packet).unwrap();
            if let Some(frame) = frame {
                sws_context
                    .scale_frame(&frame, 0, decode_context.height, &mut frame_rgb)
                    .unwrap();
                if i < 5 {
                    i += 1;
                    save_frame(
                        out_folder,
                        &frame_rgb,
                        decode_context.width,
                        decode_context.height,
                        i,
                    )
                }
            }
        }
    }
    Ok(())
}

#[test]
fn _main_test() {
    _main(
        cstr!("tests/utils/tutorial01_safe/centaur.mpg"),
        "tests/utils/tutorial01_safe/decoded/",
    )
    .unwrap();
}
