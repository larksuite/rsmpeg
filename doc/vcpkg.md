# Quick Start: Using cargo-vcpkg

Using [vcpkg](https://github.com/microsoft/vcpkg) to manage FFmpeg dependencies can be simpler because all configuration is handled within your `Cargo.toml` file. This is particularly convenient for users who download your project, as they can build all necessary dependencies by running a single command. Be aware that building FFmpeg using this method can be time-consuming, although the generated library files may be cached after the initial build.

### Install cargo-vcpkg

To begin, install the [cargo-vcpkg](https://github.com/mcgoo/cargo-vcpkg) tool:

```bash
cargo install cargo-vcpkg
```

### First rsmpeg project

After installing `cargo-vcpkg`, create a new Rust project using `cargo new <project_name>` and add `rsmpeg` to your `Cargo.toml` file. Choose the feature flag that matches your desired FFmpeg version:

```toml
[dependencies]
# For FFmpeg 6.*
rsmpeg = { version = "0.15.1", default-features = false, features = ["ffmpeg6", "link_vcpkg_ffmpeg"] }
# For FFmpeg 7.*
rsmpeg = { version = "0.15.1", default-features = false, features = ["ffmpeg7", "link_vcpkg_ffmpeg"] }
```

Add vcpkg dependencies in `Cargo.toml`:

```toml
[package.metadata.vcpkg]
git = "https://github.com/microsoft/vcpkg"
# Change the triplet according to your build target
dependencies = ["ffmpeg:x64-windows-static-md"]
# Use vcpkg's master branch
branch = "master"
```

Run the vcpkg build:

```bash
# The `--verbose` option is not mandatory but can help identify errors if the build fails.
cargo vcpkg --verbose build
```

You also need to create a `.cargo/config.toml` file in your project workspace if the build target is Windows(see [this discussion](https://github.com/larksuite/rsmpeg/pull/74#issuecomment-1085422980) for why):

```toml
[target.x86_64-pc-windows-msvc]
rustflags = [
    "-C", "link-arg=Mfplat.lib",
    "-C", "link-arg=Strmiids.lib",
    "-C", "link-arg=Mfuuid.lib",
    "-C", "link-arg=Bcrypt.lib",
    "-C", "link-arg=Secur32.lib",
    "-C", "link-arg=Ole32.lib",
    "-C", "link-arg=User32.lib"
]
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

Prepare a simple image (e.g., `./test.jpg`) in your project's root folder:

![test.jpg](./assets/mountain.jpg)

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

### Advanced configuration

To use a specific revision of vcpkg to avoid unwanted breaking changes, modify your `Cargo.toml`:

```diff
- branch = "master"
+ rev = "8cf6095" # Replace with your desired commit hash
```

You may want to specify a subset of FFmpeg features based on the modules you need. For instance, if your code uses the x264 and VPX codecs, the dependency should look like this:

```diff
- dependencies = ["ffmpeg:x64-windows-static-md"]
+ dependencies = ["ffmpeg[x264,vpx]:x64-windows-static-md"]
```

More configuration options can be found in the [`cargo-vcpkg` README](https://github.com/mcgoo/cargo-vcpkg)

