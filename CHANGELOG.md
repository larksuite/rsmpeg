## 0.11.0

- Bump `rusty_ffmpeg` version to `0.10.0`

## 0.10.0

- Added methods:
    - `AVFormatContextOutput::set_oformat`
    - `AVOutputFormat::guess_format`
    - `SwrContext::convert_frame`
    - `SwrContext::get_delay`

- `AVCodecContext::apply_codecpar` now accepts `&AVCodecParameters`

- `RsmpegError`:
    - Remove unused error variants
    - Add `RsmpegError::raw_error` for raw FFmpeg error code extraction.

## 0.9.0

- Enable metadata dict specifying in `write_header`.

- Change `get_bytes_per_sample`'s return-type from `Option<i32>` to `Option<usize>`.

- Make `AVCodecParserContext::parse_packet` respect packet's pts, dts and pos.

- Add `decode_audio` test, make existing tests robuster and faster.


## 0.8.2

- More field setter methods for `AVPacket`.

- Wrap `AVPixFmtDescriptor`.

## 0.8.1

- Bump `rusty_ffmpeg` to 0.8.1

## 0.8.0

- Supports FFmpeg 5.0

## 0.7.1

- `impl Send` for binding types.

- Dependencies version bump

## 0.7.0

- Better docs and tests (`transcoding`, `avio_writing`, `thumbnail`), more bug fixes.

- Add `av_rescale_q`, `av_rescale_q_rnd` and `ra` for easier `AVRational` manipulation.

- Add `AVFrame::make_writable`, `AVFrame::is_writable` for frame writable checking.

- Split `SwrContext::convert` into `convert` and `convert_raw` for more flexible audio api.

- Make FFmpeg's error code included in `OpenInputError`.

- Add `AVSubtitle` and related encoding and decoding api for subtitle manipulation.

- Export `UnsafeDerefMut` for users to access raw FFmpeg structure with an unsafe context.

- Allow `AVFilterGraph::parse_ptr` accept no filter inputs or outputs.

- Impl `Send` for `AVPacket` and `AVFrame`, which makes video packet processing across thread boundary possible.

## 0.6.0

- Better docs and tests (rewrites the `transcoding` and `transcoding_aac` tests), more bug fixes.

- Add `channel_layout` getter for `AVCodecContext`.

- Sync implementation between `AVImage` and `AVSamples` since they share many things in common.

- Rename `AVCodecContext::set_codecpar` to `AVCodecContext::apply_codecpar`, because this function doesn't set a field named as `codecpar` in `AVCodecContext`, but extracts the given codec parameters and set several fields of `AVCodecContext`.

- Rename `AVFrameWithImageBuffer` to `AVFrameWithImage`. And it's implementation was rewritten which aims at binding more information with it. Not it's more ergonomic.

- Wrap `av_malloc` and `av_free` in `AVMem`.

- Implment `AVIOContextUrl` and `AVIOContextCustom` for custom IO processing ability, `AVFormatContext*`'s methods changed accordingly.

## 0.5.0

- Better docs and tests.

- Add `AVBitStreamFilter` wrapper.

- Add `AVBSFContext` wrapper.

- Remove unused parameter of `AVFormatContextOutput::new_stream()`.

- Rename `RsmpegError::SendPacketAgainError` to `RsmpegError::DecoderFullError`.

## 0.4.0

- Better docs and tests.

- Add `AVMmap` wrapper for `av_file_map` related functions.

- Add convenient functions for `AVSampleFmt`.

- Better `AVSamples` methods.

- Better `SwrContext` methods.

- Fix panic when using `metadata()` of `AVStream`, `AVFormatContextInput`.

- Better metadata accessing and setting methods.

- Compatibility of `FFmpeg`'s latest master.

## 0.3.0

- First usable version.
