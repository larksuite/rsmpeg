use rsmpeg::{avcodec::AVCodec, avformat::AVFormatContextInput};
use std::ffi::{CStr, CString};
fn codec(path: &CStr) -> Vec<Option<String>> {
    AVFormatContextInput::open(path)
        .expect("Failed to load file")
        .streams()
        .into_iter()
        .map(|stream| {
            AVCodec::find_encoder(stream.codecpar().codec_id)
                .map(|codec| codec.name().to_string_lossy().to_string())
        })
        .collect()
}

#[test]
fn test_codec() {
    assert_eq!(
        codec(&CString::new("tests/assets/vids/bear.mp4").unwrap()),
        vec![Some("aac".to_string()), Some("libx264".to_string())]
    );
}
