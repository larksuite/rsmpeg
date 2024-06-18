# Rsmpeg

[![Doc](https://docs.rs/rsmpeg/badge.svg?style=flat-square)](https://docs.rs/rsmpeg)
[![Crates.io](https://img.shields.io/crates/v/rsmpeg)](https://crates.io/crates/rsmpeg)
[![CI](https://github.com/larksuite/rsmpeg/workflows/CI/badge.svg?branch=master&style=flat-square)](https://github.com/larksuite/rsmpeg/actions)

`rsmpeg` is a thin&safe layer above the FFmpeg's Rust bindings, it's main goal is safely exposing FFmpeg inner APIs in Rust as much as possible.

Taking advantage of Rust's language design, you can build robust multi-media projects even quicker than using FFmpeg's C API.

## Dependency requirements

Supported FFmpeg versions are `6.*`, `7.*`.

Minimum Supported Rust Version is `1.70.0`(stable channel).

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

These scripts build latest stable FFmpeg by default. You can build specific FFmpeg version explicitly:

```bash
# macOS & FFmpeg 7.0
zsh utils/mac_ffmpeg.rs release/7.0
```

### Compiling FFmpeg through cargo-vcpkg

Using [vcpkg](https://github.com/microsoft/vcpkg) to manage ffmpeg dependencies may be easier as all the configuration is included in your `Cargo.toml`. 
This is especially handy for users who download your project as they can build all necessary dependencies by running a single command.
Care that by using this method building ffmpeg may take a lot of time, although after the first time the generated library files may be cached. 

To begin, install the [cargo-vcpkg](https://github.com/mcgoo/cargo-vcpkg) tool:

```bash
cargo install cargo-vcpkg
```

Add vcpkg dependencies:

```rust
[package.metadata.vcpkg]
dependencies = ["ffmpeg"]
git = "https://github.com/microsoft/vcpkg"
rev = "4a600e9" // Although it is possible to link to the master branch of vcpkg, it may be better to fix a specific revision in order to avoid unwanted breaking changes.
```


You may want to specify a subset of features based on the modules of FFmpeg you need. For instance, if your code makes use of x264 and VPX codecs the dependency should look like:

```rust
dependencies = ["ffmpeg[x264,vpx]"]
```

In some cases you may need to specify the triplet and/or additional dependencies. For instance, on Windows the above section would look similar to the following:

```rust
[package.metadata.vcpkg]
dependencies = ["ffmpeg[x264,vpx]:x64-windows-static-md"]
git = "https://github.com/microsoft/vcpkg"
rev = "4a600e9"
```

The features may vary depending on your application, in our case to build the demo we need x264. 

Setup the environment: 

```bash
# *nix (the path of the folder named after the triplet may change)
export FFMPEG_PKG_CONFIG_PATH=${PWD}/target/vcpkg/installed/x64-linux/lib/pkgconfig
# Windows(CMD)
set FFMPEG_PKG_CONFIG_PATH=%CD%\target\vcpkg\installed\x64-windows-static-md\lib\pkgconfig
# Windows(PowerShell)
$env:FFMPEG_PKG_CONFIG_PATH="$(($PWD).path)\target\vcpkg\installed\x64-windows-static-md\lib\pkgconfig"
```

Run the vcpkg build:
```bash
cargo vcpkg --verbose build
```
The `--verbose` option is not mandatory but may help to recognize any error in case the build fails.

After those steps you are able to build and run your project. A full working example with the demo code presented in the next section is available at https://github.com/aegroto/rsmpeg-vcpkg-demo.


### Rsmpeg demo

Ensure that you have compiled the FFmpeg.

Start by adding `rsmpeg` to your `Cargo.toml` file:

```toml
[dependencies]
# Add this if you are using ffmpeg 6.*
rsmpeg = { version = "0.15.1", default-features = false, features = ["ffmpeg6"] }
# Add this if you are using ffmpeg 7.* (feature `ffmpeg7` is enabled by default)
rsmpeg = "0.15.1"
```

Write your simple media file info dumper:

```rust
use std::ffi::{CStr, CString};
use std::error::Error;
use rsmpeg::avformat::AVFormatContextInput;

fn dump_av_info(path: &CStr) -> Result<(), Box<dyn Error>> {
    let mut input_format_context = AVFormatContextInput::open(path, None, &mut None)?;
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

2. Advanced usage of rsmpeg: Check out the `tests` folder.

## Contributors

Thanks for your contributions!

+ [@vnghia](https://github.com/vnghia)
+ [@nxtn](https://github.com/nxtn)
+ [@aegroto](https://github.com/aegroto)
+ [@nanpuyue](https://github.com/nanpuyue)
+ [@imxood](https://github.com/imxood)
+ [@FallingSnow](https://github.com/FallingSnow)
+ [@Jamyw7g](https://github.com/Jamyw7g)
