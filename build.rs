fn find_rc_compiler() -> Option<String> {
    if std::env::var("RC").is_ok() {
        return None;
    }
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let candidates: &[&str] = if target_env == "gnu" {
        &[
            "x86_64-w64-mingw32-windres",
            "/usr/bin/x86_64-w64-mingw32-windres",
            "windres",
            "llvm-rc",
        ]
    } else {
        &[
            "llvm-rc",
            "/usr/bin/llvm-rc-19",
            "/usr/bin/llvm-rc-18",
            "/usr/bin/llvm-rc-17",
            "/usr/bin/llvm-rc",
            "x86_64-w64-mingw32-windres",
            "/usr/bin/x86_64-w64-mingw32-windres",
            "windres",
        ]
    };
    for &cand in candidates {
        if std::path::Path::new(cand).is_file() {
            return Some(cand.to_string());
        }
        if std::process::Command::new(cand).arg("/?").output().is_ok()
            || std::process::Command::new(cand).arg("-V").output().is_ok()
        {
            return Some(cand.to_string());
        }
    }
    None
}

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        if let Some(rc_path) = find_rc_compiler() {
            unsafe {
                std::env::set_var("RC", rc_path);
            }
        }
        println!("cargo:rerun-if-changed=clipper.rc");
        println!("cargo:rerun-if-changed=assets/app.ico");
        println!("cargo:rerun-if-changed=assets/app_inverted.ico");
        embed_resource::compile("clipper.rc", embed_resource::NONE);
    }
}
