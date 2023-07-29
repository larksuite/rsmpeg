use cstr::cstr;
use rsmpeg::avformat::*;
use std::ffi::CStr;

/// Dump video/audio/image info to stdout.
fn image_dump(image_path: &CStr) -> Result<(), Box<dyn std::error::Error>> {
    let mut input_format_context = AVFormatContextInput::open(image_path, None, &mut None)?;
    input_format_context.dump(0, image_path)?;
    Ok(())
}

#[test]
fn image_test() {
    image_dump(cstr!("tests/assets/pics/bear.jpg")).unwrap();
    image_dump(cstr!("tests/assets/pics/gif.webp")).unwrap();
    image_dump(cstr!("tests/assets/pics/mail.jpg")).unwrap();
    image_dump(cstr!("tests/assets/pics/mountain.jpg")).unwrap();
    image_dump(cstr!("tests/assets/pics/pink.jpg")).unwrap();
    image_dump(cstr!("tests/assets/pics/redwine.jpg")).unwrap();
    image_dump(cstr!("tests/assets/pics/sea.jpg")).unwrap();
}
