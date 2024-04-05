#!/bin/sh
#![allow(unused_attributes)] /*
OUT=/tmp/tmp && rustc "$0" -o ${OUT} && exec ${OUT} $@ || exit $? #*/

use std::process::Command;
use std::io::Result;
use std::path::PathBuf;
use std::fs;

fn mkdir(dir_name: &str) -> Result<()> {
    fs::create_dir(dir_name)
}

fn pwd() -> Result<PathBuf> {
    std::env::current_dir()
}

fn cd(dir_name: &str) -> Result<()> {
    std::env::set_current_dir(dir_name)
}

fn main() -> Result<()> {
    let _ = mkdir("tmp");

    cd("tmp")?;

    let tmp_path = pwd()?.to_string_lossy().to_string();
    let build_path = format!("{}/ffmpeg_build", tmp_path);
    let branch = std::env::args().nth(1).unwrap_or_else(|| "release/7.0".to_string());
    let num_job = std::thread::available_parallelism().unwrap().get();

    if fs::metadata("ffmpeg").is_err() {
        Command::new("git")
            .arg("clone")
            .arg("--single-branch")
            .arg("--branch")
            .arg(&branch)
            .arg("--depth")
            .arg("1")
            .arg("https://github.com/ffmpeg/ffmpeg")
            .status()?;
    }

    cd("ffmpeg")?;

    Command::new("git")
        .arg("fetch")
        .arg("origin")
        .arg(&branch)
        .arg("--depth")
        .arg("1")
        .status()?;

    Command::new("git")
        .arg("checkout")
        .arg("FETCH_HEAD")
        .status()?;

    Command::new("./configure")
        .arg(format!("--prefix={}", build_path))
        .arg("--enable-gpl")
        // .arg("--enable-libass")
        // .arg("--enable-libfdk-aac")
        // .arg("--enable-libfreetype")
        // .arg("--enable-libmp3lame")
        // .arg("--enable-libopus")
        // .arg("--enable-libvorbis")
        .arg("--enable-libvpx")
        .arg("--enable-libx264")
        // .arg("--enable-libx265")
        // To workaround `https://github.com/larksuite/rsmpeg/pull/98#issuecomment-1467511193`
        .arg("--disable-decoder=exr,phm")
        .arg("--disable-programs")
        .arg("--enable-nonfree")
        .status()?;

    Command::new("make")
        .arg("-j")
        .arg(num_job.to_string())
        .status()?;

    Command::new("make")
        .arg("install")
        .status()?;

    cd("..")?;

    Ok(())
}
