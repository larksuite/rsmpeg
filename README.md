# Rsmpeg

`rsmpeg` is a thin&safe layer above the raw FFmpeg's Rust binding, it's main goal is safely exposing FFmpeg inner APIs in Rust as much as possible.

Taking advantage of Rust's language design, you can build robust multi-media project even quicker than using FFmpeg's C API.

## Getting started

### FFmpeg compilation

To use your first rsmpeg demo, you need to compile your FFmpeg:
1. <https://github.com/ffmpeg/ffmpeg>.
2. <https://trac.ffmpeg.org/wiki/CompilationGuide>

If you found the compilation compilcated, there are some helpful compiling scripts for you(in `utils` folder).

To build a FFmpeg with default parameters: (don't forget to install the build dependencies)

```bash
# macOS
zsh utils/mac_ffmpeg.rs
# Linux
bash utils/linux_ffmpeg.rs
# Windows
# You need a Linux machine for cross compiling, then copy the artifact to your
# Windows machine.
bash utils/windows_ffmpeg.rs
```

### Rsmpeg demo

Ensure that you have compiled the FFmpeg.

Start by adding `rsmpeg` to your `Cargo.toml` file:

```rust
[dependencies]
rsmpeg = "0.2.0"
```

Write your simple image info dumper:

```rust
use std::ffi::{CStr, CString};
use std::error::Error;
use rsmpeg::avformat::AVFormatContextInput;

fn dump_image_info(image_path: &CStr) -> Result<(), Box<dyn Error>> {
    let mut input_format_context = AVFormatContextInput::open(image_path)?;
    input_format_context.dump(0, image_path)?;
    Ok(())
}

fn main() {
    dump_image_info(&CString::new("./test.jpg").unwrap()).unwrap();
}
```

Prepare an simple image in your current folder:

![test.jpg](./assets/mountain.jpg)

Run with `FFMPEG_PKG_CONFIG_PATH` set to the pkgconfig file path in your artifact folder(`xxx/lib/pkgconfig`).

```bash
# macOS
export FFMPEG_PKG_CONFIG_PATH=xxx/ffmpeg_build/lib/pkgconfig
# Windows
set FFMPEG_PKG_CONFIG_PATH=xxx/ffmpeg_build/lib/pkgconfig

cargo run
```

Then it works:

```
Input #0, image2, from './test.jpg':
  Duration: 00:00:00.04, start: 0.000000, bitrate: 1390 kb/s
  Stream #0:0: Video: mjpeg, none, 25 fps, 25 tbr, 25 tbn, 25 tbc
```
