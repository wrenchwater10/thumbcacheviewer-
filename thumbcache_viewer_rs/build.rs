// Embeds the application manifest (visual styles + DPI awareness) and the icon
// with windres when a mingw toolchain is available. The build still succeeds
// without it, just without the icon and common-controls v6 styling.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=res/app.rc");
    println!("cargo:rerun-if-changed=res/app.manifest");
    println!("cargo:rerun-if-changed=res/icon.ico");
    println!("cargo:rerun-if-env-changed=WINDRES");

    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("windows") {
        return;
    }

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let obj = format!("{}/app_rc.o", out_dir);
    let windres = std::env::var("WINDRES").unwrap_or_else(|_| {
        if target.starts_with("i686") {
            "i686-w64-mingw32-windres".to_string()
        } else {
            "x86_64-w64-mingw32-windres".to_string()
        }
    });

    let status = Command::new(&windres)
        .arg("-I")
        .arg("res")
        .arg("res/app.rc")
        .arg("-O")
        .arg("coff")
        .arg("-o")
        .arg(&obj)
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("cargo:rustc-link-arg={}", obj);
        }
        _ => {
            println!(
                "cargo:warning={} was not usable; building without icon and manifest",
                windres
            );
        }
    }
}
