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
