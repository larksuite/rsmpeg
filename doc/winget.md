# Quick Start: Using a winget (BtbN) FFmpeg Installation

> **NOTE:** This method installs FFmpeg for your user. If you already have FFmpeg installed, either for your user or system-wide, using this method will lead to headaches. We recommend using this method only for development setups where this will be your only installation of FFmpeg.

You will need to have winget installed. It may be pre-installed on certain versions of Windows. See the [official documentation](https://learn.microsoft.com/en-us/windows/package-manager/winget/) for installation instructions.

> **NOTE:** The scripts in this tutorial are for Powershell.

### Installing FFmpeg

Find the version of FFmpeg that you would like to install: 
```powershell
winget search BtbN.FFmpeg
```

> **NOTE:** Be sure to select a Shared variant, as only those contain the necessary header and library files.

Install your desired version. We will use GPL 7.1 for this example:
```powershell
winget install --id BtbN.FFmpeg.GPL.Shared.7.1
```

### Preparing the environment

```powershell
# Helper variable: root dir of BtbN.FFmpeg installation
$FFMPEG_HOME = (Get-Command ffmpeg).Path | Join-Path -ChildPath "..\.." | Resolve-Path

# Required rusty_ffmpeg configuration
$env:FFMPEG_LIBS_DIR = Join-Path $FFMPEG_HOME lib
$env:FFMPEG_INCLUDE_DIR = Join-Path $FFMPEG_HOME include
```

### First rsmpeg project

Add `rsmpeg` to your `Cargo.toml` file. Choose the feature flag that matches your installed FFmpeg version:

```toml
# FFmpeg 6.*
rsmpeg = { version = "0.18", default-features = false, features = ["ffmpeg6"] }
# FFmpeg 7.*
rsmpeg = { version = "0.18", default-features = false, features = ["ffmpeg7"] }
# FFmpeg 8.* (feature `ffmpeg8` is enabled by default)
rsmpeg = "0.18"
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

Run your project in the environment prepared above:

```powershell
cargo run
```

Expected output:

```output
Input #0, image2, from './test.jpg':
  Duration: 00:00:00.04, start: 0.000000, bitrate: 1390 kb/s
  Stream #0:0: Video: mjpeg (Progressive), yuvj420p(pc, bt470bg/unknown/unknown), 400x300 [SAR 72:72 DAR 4:3], 25 fps, 25 tbr, 25 tbn
```

(Note: A single image's duration is 0.04s when processed at 25 frames per second.)

You can also use any video or audio file with this program, and it will dump the media information for that file.