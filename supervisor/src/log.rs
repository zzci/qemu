//! Tiny logger matching the old bash `[tag] msg` style (blue tag, red for errors).

pub fn info(msg: impl AsRef<str>) {
    println!("\x1b[1;34m[vmd]\x1b[0m {}", msg.as_ref());
}

pub fn warn(msg: impl AsRef<str>) {
    eprintln!("\x1b[1;33m[vmd]\x1b[0m {}", msg.as_ref());
}

pub fn error(msg: impl AsRef<str>) {
    eprintln!("\x1b[1;31m[vmd] !!\x1b[0m {}", msg.as_ref());
}
