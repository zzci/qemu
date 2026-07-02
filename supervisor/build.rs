//! Guarantee ui/dist exists for `include_dir!` — a bare local build gets a placeholder page.
use std::path::Path;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let dist = Path::new(&manifest).join("ui/dist");
    if !dist.join("index.html").exists() {
        let _ = std::fs::create_dir_all(&dist);
        let _ = std::fs::write(
            dist.join("index.html"),
            "<!doctype html><meta charset=utf-8><title>vmd</title>\
             <body style=\"font-family:system-ui;background:#0b0e14;color:#c9d1d9\">\
             web console not built — run <code>npm --prefix ui ci &amp;&amp; npm --prefix ui run build</code></body>",
        );
    }
    println!("cargo:rerun-if-changed=ui/dist");
}
