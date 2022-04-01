fn main() {
    build();
}

#[cfg(not(target_os = "windows"))]
fn build() {}

#[cfg(all(target_os = "windows", target_env = "gnu"))]
fn build() {}

#[cfg(all(target_os = "windows", target_env = "msvc"))]
fn build() {
    /* for ffmpeg */
    let extra_libs = [
        "dxva2",
        "evr",
        "mf",
        "mfplat",
        "mfplay",
        "mfreadwrite",
        "mfuuid",
        "bcrypt",
        "ws2_32",
        "Secur32",
        "Strmiids",
        "ole32",
        "user32",
    ];

    for lib in extra_libs.iter() {
        println!("cargo:rustc-flags=-l {}", &lib);
    }
}
