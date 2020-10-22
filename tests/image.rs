use rsmpeg::avformat::*;

use std::ffi::{CStr, CString};

fn image(image_path: &CStr) -> Result<(), Box<dyn std::error::Error>> {
    let mut input_format_context = AVFormatContextInput::open(image_path)?;
    input_format_context.dump(0, image_path)?;
    Ok(())
}

#[test]
fn image_test() {
    image(&CString::new("tests/utils/image/a.jpg").unwrap()).unwrap();
    image(&CString::new("tests/utils/image/b.jpg").unwrap()).unwrap();
    image(&CString::new("tests/utils/image/c.jpg").unwrap()).unwrap();
    image(&CString::new("tests/utils/image/d.jpg").unwrap()).unwrap();
    image(&CString::new("tests/utils/image/e.jpg").unwrap()).unwrap();
    image(&CString::new("tests/utils/image/f.jpg").unwrap()).unwrap();
    image(&CString::new("tests/utils/image/gif.webp").unwrap()).unwrap();
}
