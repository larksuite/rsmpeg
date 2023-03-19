#!/bin/sh
#![allow()] /*
OUT=/tmp/tmp && rustc "$0" -o ${OUT} && exec ${OUT} $@ || exit $? #*/

use std::process::Command;
use std::io::Result;
use std::path::PathBuf;

fn mkdir(dir_name: &str) -> Result<()> {
    Command::new("mkdir")
        .arg(dir_name)
        .spawn()?
        .wait()?;
    Ok(())
}

fn pwd() -> Result<PathBuf> {
    std::env::current_dir()
}

fn cd(dir_name: &str) -> Result<()> {
    let mut current_dir = pwd()?;
    current_dir.push(dir_name);
    std::env::set_current_dir(current_dir)?;
    Ok(())
}

fn main() -> Result<()> {
    mkdir("tmp")?;
    cd("tmp")?;

    let tmp_path = pwd()?;
    let tmp_path = tmp_path.to_str().unwrap();

    Command::new("git")
        .arg("clone")
        .arg("--single-branch")
        .arg("--branch")
        .arg("release/6.0")
        .arg("--depth")
        .arg("1")
        .arg("https://github.com/ffmpeg/ffmpeg")
        .spawn()?
        .wait()?;

    cd("ffmpeg")?;

    Command::new("./configure")
        .arg(format!("--prefix={}/ffmpeg_build", tmp_path))
        .arg("--enable-gpl")
        // .arg("--enable-libass")
        // .arg("--enable-libfdk-aac")
        // .arg("--enable-libfreetype")
        // .arg("--enable-libmp3lame")
        // .arg("--enable-libopus")
        // .arg("--enable-libvorbis")
        // .arg("--enable-libvpx")
        // .arg("--enable-libx264")
        // .arg("--enable-libx265")
        // To workaround `https://github.com/larksuite/rsmpeg/pull/98#issuecomment-1467511193`
        .arg("--disable-decoder=exr,phm")
        .arg("--disable-programs")
        .arg("--enable-nonfree")
        .arg("--arch=x86")
        .arg("--target-os=mingw32")
        .arg("--cross-prefix=i686-w64-mingw32-")
        .arg("--pkg-config=pkg-config")
        .spawn()?
        .wait()?;

    Command::new("make")
        .arg("-j")
        .arg(std::thread::available_parallelism().unwrap().get().to_string())
        .spawn()?
        .wait()?;

    Command::new("make")
        .arg("install")
        .spawn()?
        .wait()?;

    cd("..")?;

    Ok(())
}
