# Quick Start: Using a System-Wide FFmpeg Installation

If you have installed FFmpeg using a package manager, such as `apt` on Debian/Ubuntu or `brew` on macOS, you can configure `rsmpeg` to link against this system-wide FFmpeg installation.

This approach bypasses the need to compile FFmpeg from source, which can be time-consuming. It ensures that your project uses the FFmpeg version managed by your system's package manager.

### Install system-wide FFmpeg

Before proceeding, ensure that you have the FFmpeg libraries and the corresponding development headers installed on your system. The package names can vary depending on your distribution.

**Debian/Ubuntu:**
```bash
sudo apt update
sudo apt install libavcodec-dev libavdevice-dev libavfilter-dev libavformat-dev libavutil-dev libpostproc-dev libswresample-dev libswscale-dev pkg-config
```

**macOS (using Homebrew):**
```zsh
brew install ffmpeg pkg-config
```

Ensure `pkg-config` is installed, as it is used to find the FFmpeg libraries. Also, verify that `pkg-config` can find your FFmpeg installation. You can test this with:
```bash
pkg-config --modversion libavcodec
```
This command should output the version of your installed libavcodec.

### First rsmpeg project

After that, create a new Rust project using `cargo new <project_name>` and add `rsmpeg` to your `Cargo.toml` file. Choose the feature flag that matches your system's FFmpeg version:

```toml
[dependencies]
# For FFmpeg 6.*
rsmpeg = { version = "0.16", default-features = false, features = ["ffmpeg6", "link_system_ffmpeg"] }
# For FFmpeg 7.*
rsmpeg = { version = "0.16", default-features = false, features = ["ffmpeg7", "link_system_ffmpeg"] }
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

![test.jpg](../assets/mountain.jpg)

After completing these steps, you can build and run your project:

```bash
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
