# Quick Start: Building FFmpeg from source

To use your first `rsmpeg` demo, you need to compile FFmpeg. You can find the official FFmpeg source code and compilation guides here:
1. <https://github.com/ffmpeg/ffmpeg>
2. <https://trac.ffmpeg.org/wiki/CompilationGuide>

If you find the official compilation process complicated, this project provides helpful scripts in the `utils` folder to simplify building FFmpeg.

To build FFmpeg with common parameters using these scripts (ensure you have installed the necessary build dependencies first):

```bash
# macOS
zsh utils/mac_ffmpeg.rs
# Linux
bash utils/linux_ffmpeg.rs
# Windows
# Cross compiling on a Linux machine or WSL, then copy the artifacts to your Windows machine.
bash utils/windows_ffmpeg.rs
```

These scripts build the latest stable FFmpeg by default. You can also build a specific FFmpeg version explicitly:

```bash
# macOS & FFmpeg 7.0
zsh utils/mac_ffmpeg.rs release/7.0
```

### First rsmpeg project

Ensure that you have successfully compiled FFmpeg.

Start by adding `rsmpeg` to your `Cargo.toml` file. Choose the feature flag that matches your compiled FFmpeg version:

```toml
[dependencies]
# FFmpeg 6.*
rsmpeg = { version = "0.15.1", default-features = false, features = ["ffmpeg6"] }
# FFmpeg 7.* (feature `ffmpeg7` is enabled by default)
rsmpeg = "0.15.1"
```

Write your simple media file info dumper in `src/main.rs`:

```rust
use std::ffi::CStr;
use std::error::Error;
use rsmpeg::avformat::AVFormatContextInput;

fn dump_av_info(path: &CStr) -> Result<(), Box<dyn Error>> {
    let mut input_format_context = AVFormatContextInput::open(path, None, &mut None)?;
    input_format_context.dump(0, path)?;
    Ok(())
}

fn main() {
    dump_av_info(c"./test.jpg").unwrap();
}
```

Prepare a simple image (`./test.jpg`) in your project's root folder:

![test.jpg](./assets/mountain.jpg)

Run your project with the `FFMPEG_PKG_CONFIG_PATH` environment variable set to the `pkgconfig` directory within your FFmpeg build artifacts folder:

```bash
# If you used the scripts from the `utils` folder, the `pkgconfig` directory is typically located at `{PWD}/tmp/ffmpeg_build/lib/pkgconfig`.
# If you built FFmpeg manually, ensure that `FFMPEG_PKG_CONFIG_PATH` points to the absolute path of your `pkgconfig` directory.

# macOS & Linux
export FFMPEG_PKG_CONFIG_PATH=${PWD}/tmp/ffmpeg_build/lib/pkgconfig
# Windows(CMD)
set FFMPEG_PKG_CONFIG_PATH="%CD%\tmp\ffmpeg_build\lib\pkgconfig"
# Windows(PowerShell)
$env:FFMPEG_PKG_CONFIG_PATH="$(($PWD).path)\tmp\ffmpeg_build\lib\pkgconfig"

cargo run
```

Expected output:

```output
[mjpeg @ 0000021AA6D1BF40] EOI missing, emulating
Input #0, image2, from './test.jpg':
  Duration: 00:00:00.04, start: 0.000000, bitrate: 1390 kb/s
  Stream #0:0: Video: mjpeg (Progressive), yuvj420p(pc, bt470bg/unknown/unknown), 400x300 [SAR 72:72 DAR 4:3], 25 fps, 25 tbr, 25 tbn
```

(Note: A single image's duration is 0.04s when processed at 25 frames per second.)

You can also use any video or audio file with this program, and it will dump the media information for that file.
