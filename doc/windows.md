# Self-built

### Using vcpkg
Using vcpkg to install FFmpeg is the easiest way to get a self-built FFmpeg on Windows: [tutorial](./vcpkg.md)

### Cross compilation
If you use rsmpeg in production on Windows, I highly recommend you to cross compile FFmpeg from source on a Linux machine or WSL. As it allows you to tweak more configure script options: [tutorial](./source-build.md)

# Using BtbN's builds
[BtbN](https://github.com/BtbN/FFmpeg-Builds) provides easily accessible builds of FFmpeg for FFmpeg with a plethora of features enabled. The Shared versions of these builds include all necessary headers and libraries to get started.

### Winget
The easiest way to install these builds is with winget: [tutorial](./winget.md)

### Manual Installation
- Download a **Shared** Windows build from [BtbN's github releases](https://github.com/BtbN/FFmpeg-Builds/releases).
- Unzip it somewhere.
- Point your FFMPEG_LIBS_DIR, FFMPEG_INCLUDE_DIR, and PATH environment variables the lib, include, and bin directories, respectively.
- See the "First rsmpeg project" section of the [winget tutorial](./winget.md).
