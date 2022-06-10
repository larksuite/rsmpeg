use anyhow::{Context, Result};
use cstr::cstr;
use rsmpeg::{avcodec::AVCodecContext, avformat::AVFormatContextInput, avutil::av_q2d, ffi};
use std::ffi::CString;

/// Get metadata key-value pair form a video file.
fn metadata(file: &str) -> Result<Vec<(String, String)>> {
    let mut result = vec![];
    result.push(("image_path".into(), file.to_string()));

    let file = CString::new(file).unwrap();
    let input_format_context = AVFormatContextInput::open(&file).unwrap();

    // Get `duration` and `bit_rate` from `input_format_context`.
    result.push(("duration".into(), input_format_context.duration.to_string()));
    result.push(("bit_rate".into(), input_format_context.bit_rate.to_string()));

    // Get additional info from `input_format_context.metadata()`
    if let Some(metadata) = input_format_context.metadata() {
        let mut prev_entry = None;

        // Trick to get all entries.
        while let Some(entry) = metadata.get(cstr!(""), prev_entry, ffi::AV_DICT_IGNORE_SUFFIX) {
            result.push((
                entry.key().to_str().unwrap().to_string(),
                entry.value().to_str().unwrap().to_string(),
            ));
            prev_entry = Some(entry);
        }
    }

    {
        // Get `frame_rate` from `video_stream`
        let (video_stream_index, decoder) = input_format_context
            .find_best_stream(ffi::AVMediaType_AVMEDIA_TYPE_VIDEO)?
            .context("Failed to find video stream")?;

        let video_stream = input_format_context
            .streams()
            .get(video_stream_index)
            .unwrap();

        result.push((
            "frame_rate".into(),
            av_q2d(video_stream.r_frame_rate).to_string(),
        ));

        // Get `width` and `height` from `decode_context`
        let mut decode_context = AVCodecContext::new(&decoder);
        decode_context
            .apply_codecpar(&video_stream.codecpar())
            .unwrap();
        decode_context.open(None).unwrap();
        result.push(("width".into(), decode_context.width.to_string()));
        result.push(("height".into(), decode_context.height.to_string()));
    };

    Ok(result)
}

#[test]
fn metadata_test0() {
    assert_eq!(
        metadata("tests/assets/vids/bear.mp4").unwrap(),
        vec![
            ("image_path".into(), "tests/assets/vids/bear.mp4".into()),
            ("duration".into(), "1066667".into()),
            ("bit_rate".into(), "308242".into()),
            ("major_brand".into(), "isom".into()),
            ("minor_version".into(), "1".into()),
            ("compatible_brands".into(), "isomavc1".into()),
            ("creation_time".into(), "2009-07-09T17:29:47.000000Z".into()),
            ("frame_rate".into(), "29.97002997002997".into()),
            ("width".into(), "320".into()),
            ("height".into(), "180".into()),
        ]
    );
}

#[test]
fn metadata_test1() {
    assert_eq!(
        metadata("tests/assets/vids/vp8.mp4").unwrap(),
        vec![
            ("image_path".into(), "tests/assets/vids/vp8.mp4".into()),
            ("duration".into(), "17600000".into()),
            ("bit_rate".into(), "242823".into()),
            ("encoder".into(), "whammy".into()),
            ("frame_rate".into(), "5".into()),
            ("width".into(), "604".into()),
            ("height".into(), "604".into()),
        ]
    );
}
