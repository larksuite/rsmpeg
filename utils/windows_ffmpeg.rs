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

fn cp(a: &str, b: &str) -> Result<()> {
    let mut from = pwd()?;
    from.push(a);
    let mut to = pwd()?;
    to.push(b);

    Command::new("cp")
        .arg(from)
        .arg(to)
        .spawn()?
        .wait()?;
    Ok(())
}

fn git_clone(url: &str) -> Result<()> {
    Command::new("git")
        .arg("clone")
        .arg(url)
        .spawn()?
        .wait()?;
    Ok(())
}

fn git_checkout(branch: &str) -> Result<()> {
    Command::new("git")
        .arg("checkout")
        .arg(branch)
        .spawn()?
        .wait()?;
    Ok(())
}

fn main() -> Result<()> {
    mkdir("tmp")?;
    cd("tmp")?;

    let tmp_path = pwd()?;
    let tmp_path = tmp_path.to_str().unwrap();

    git_clone("https://github.com/ffmpeg/ffmpeg")?;
    cd("ffmpeg")?;
    git_checkout("origin/release/4.4")?;
    cd("..")?;

    cd("ffmpeg")?;
    {
        Command::new("./configure")
            .arg(format!("--prefix={}/ffmpeg_build", tmp_path))
            .arg("--arch=x86")
            .arg("--target-os=mingw32")
            .arg("--cross-prefix=i686-w64-mingw32-")
            .arg("--pkg-config=pkg-config")
            .spawn()?
            .wait()?;

        Command::new("make")
            .arg("-j8")
            .spawn()?
            .wait()?;

        Command::new("make")
            .arg("install")
            .spawn()?
            .wait()?;
    }
    cd("..")?;

    Ok(())
}