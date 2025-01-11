use std::{env, fs, io, os, path::PathBuf};

pub fn get_home_folder() -> PathBuf {
    // note: using .expect(...) here because these environment variables are meant to be set
    // by the login process of either OS, so if they are not set, something is very wrong
    PathBuf::from(if cfg!(target_family = "windows") {
        env::var("USERPROFILE").expect("%USERPROFILE% environment not set")
    } else {
        env::var("HOME").expect("$HOME environment not set")
    })
}

pub fn get_ssh_path() -> PathBuf {
    get_home_folder().join(".ssh")
}

pub fn soft_link(from: &PathBuf, to: &PathBuf) -> Result<(), io::Error> {
    if to.is_symlink() {
        fs::remove_file(to)?;
    }

    #[cfg(target_family = "windows")]
    os::windows::fs::symlink_file(from, to)?;

    #[cfg(target_family = "unix")]
    os::unix::fs::symlink(from, to)?;

    Ok(())
}
