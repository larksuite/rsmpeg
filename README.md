# Rsmpeg

[![Doc](https://docs.rs/rsmpeg/badge.svg?style=flat-square)](https://docs.rs/rsmpeg)
[![Crates.io](https://img.shields.io/crates/v/rsmpeg)](https://crates.io/crates/rsmpeg)
[![CI](https://github.com/larksuite/rsmpeg/workflows/CI/badge.svg?branch=master&style=flat-square)](https://github.com/larksuite/rsmpeg/actions)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/larksuite/rsmpeg)

`rsmpeg` is a thin and safe layer above the FFmpeg's Rust bindings, its main goal is to safely expose FFmpeg inner APIs in Rust as much as possible.

Taking advantage of Rust's language design, you can build robust multi-media projects even quicker than using FFmpeg's C API.

## Dependency requirements

Supported FFmpeg versions are `6.*`, `7.*`.

Minimum Supported Rust Version is `1.77.0`(stable channel).

## Getting started

[Windows users](./doc/windows.md)

[macOS, Linux users](./doc/non-windows.md)

## Advanced usage

1. Advanced FFmpeg linking: refer to [`rusty_ffmpeg`](https://github.com/CCExtractor/rusty_ffmpeg)'s documentation for how to use environment variables to statically or dynamically link FFmpeg. `rsmpeg` also mirrors `rusty_ffmpeg`'s `link_system_ffmpeg` and `link_vcpkg_ffmpeg` features for you to use ffmpeg installed by package manager (e.g., `apt`, `brew`, `vcpkg`).

2. rsmpeg examples: Check out the [`tests/ffmpeg_examples` folder](./tests/ffmpeg_examples/), which partially mirrors [ffmpeg examples](https://github.com/FFmpeg/FFmpeg/tree/master/doc/examples).

## Contributors

Thanks for your contributions!

+ [@Yesterday17](https://github.com/Yesterday17)
+ [@vnghia](https://github.com/vnghia)
+ [@nxtn](https://github.com/nxtn)
+ [@aegroto](https://github.com/aegroto)
+ [@nanpuyue](https://github.com/nanpuyue)
+ [@imxood](https://github.com/imxood)
+ [@FallingSnow](https://github.com/FallingSnow)
+ [@Jamyw7g](https://github.com/Jamyw7g)
