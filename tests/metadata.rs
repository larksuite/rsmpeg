use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext},
    avformat::AVFormatContextInput,
    ffi,
};
use std::ffi::CString;

fn metadata(file: &str) -> Vec<(String, String)> {
    let mut result = vec![];
    result.push(("image_path".into(), file.to_string()));

    let file = CString::new(file).unwrap();
    let input_format_context = AVFormatContextInput::open(&file).unwrap();

    result.push(("duration".into(), input_format_context.duration.to_string()));
    result.push(("bit_rate".into(), input_format_context.bit_rate.to_string()));
    {
        let metadata = input_format_context.metadata();
        let mut prev_entry = None;
        while let Some(entry) = metadata.get(
            &CString::new("").unwrap(),
            prev_entry,
            ffi::AV_DICT_IGNORE_SUFFIX,
        ) {
            result.push((
                entry.key().to_str().unwrap().to_string(),
                entry.value().to_str().unwrap().to_string(),
            ));
            prev_entry = Some(entry);
        }
    }
    {
        let video_stream = input_format_context
            .streams()
            .into_iter()
            .find(|stream| stream.codecpar().codec_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO)
            .unwrap();
        result.push((
            "frame_rate".into(),
            ffi::av_q2d(video_stream.r_frame_rate).to_string(),
        ));
        let decoder = AVCodec::find_decoder(video_stream.codecpar().codec_id).unwrap();
        let mut decode_context = AVCodecContext::new(&decoder);
        decode_context
            .set_codecpar(video_stream.codecpar())
            .unwrap();
        decode_context.open(None).unwrap();
        result.push(("width".into(), decode_context.width.to_string()));
        result.push(("height".into(), decode_context.height.to_string()));
    };
    result
}

#[test]
fn metadata_test() {
    assert_eq!(
        metadata("tests/utils/metadata/bear.mp4"),
        vec![
            ("image_path".into(), "tests/utils/metadata/bear.mp4".into()),
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
