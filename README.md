# Rsmpeg

[![Doc](https://docs.rs/rsmpeg/badge.svg?style=flat-square)](https://docs.rs/rsmpeg)
[![Crates.io](https://img.shields.io/crates/v/rsmpeg)](https://crates.io/crates/rsmpeg)
[![CI](https://github.com/larksuite/rsmpeg/workflows/CI/badge.svg?branch=master&style=flat-square)](https://github.com/larksuite/rsmpeg/actions)

`rsmpeg` is a thin&safe layer above the FFmpeg's Rust bindings, it's main goal is safely exposing FFmpeg inner APIs in Rust as much as possible.

Taking advantage of Rust's language design, you can build robust multi-media projects even quicker than using FFmpeg's C API.

## Getting started

### FFmpeg compilation

To use your first rsmpeg demo, you need to compile your FFmpeg:
1. <https://github.com/ffmpeg/ffmpeg>.
2. <https://trac.ffmpeg.org/wiki/CompilationGuide>

If you find the compilation complicated, there are some helpful compiling scripts for you (under the `utils` folder).

To build a FFmpeg with some common parameters: (don't forget to install the build dependencies)

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
rsmpeg = "0.11"
```

Write your simple media file info dumper:

```rust
use std::ffi::{CStr, CString};
use std::error::Error;
use rsmpeg::avformat::AVFormatContextInput;

fn dump_av_info(path: &CStr) -> Result<(), Box<dyn Error>> {
    let mut input_format_context = AVFormatContextInput::open(path)?;
    input_format_context.dump(0, path)?;
    Ok(())
}

fn main() {
    dump_av_info(&CString::new("./test.jpg").unwrap()).unwrap();
}
```

Prepare a simple image in your current folder:

![test.jpg](./assets/mountain.jpg)

Run with `FFMPEG_PKG_CONFIG_PATH` set to the pkgconfig file path (**Absolute path!**) in your artifact folder (`xxx/ffmpeg_build/lib/pkgconfig`).

```bash
# macOS & Linux
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

(A single image's duration under 25fps is 0.04s)

You can also put any video or audio file here, this program will dump the media info for you.

## Advanced usage

1. FFmpeg linking: refer to [`rusty_ffmpeg`](https://github.com/CCExtractor/rusty_ffmpeg)'s documentation for how to use environment variables to statically or dynamically link FFmpeg.

2. Advanced usage of rsmpeg: Check out the `tests` and `examples` folder.

## Dependency version

Supported FFmpeg version is 5.0.

Minimum Supported Rust Version is 1.56(Stable channel).

## Contributors

Thanks for your contributions!

+ [@nxtn](https://github.com/nxtn)
+ [@aegroto](https://github.com/aegroto)
+ [@nanpuyue](https://github.com/nanpuyue)
+ [@imxood](https://github.com/imxood)
+ [@FallingSnow](https://github.com/FallingSnow)
+ [@Jamyw7g](https://github.com/Jamyw7g)
