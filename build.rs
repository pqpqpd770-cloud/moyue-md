use std::path::{Path, PathBuf};
use std::process::Command;

/// Compile assets/app.rc (icon + version info) into a .res and hand it to the linker.
/// rc.exe is looked up inside the installed Windows SDK; a missing SDK only costs us
/// the embedded icon, never the build itself.
fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let rc_file = manifest.join("assets").join("app.rc");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let res_file = out_dir.join("moyue.res");

    let Some(rc_exe) = find_rc() else {
        println!("cargo:warning=rc.exe not found; building without an embedded icon resource");
        return;
    };

    let status = Command::new(&rc_exe)
        .arg("/nologo")
        .arg("/c65001")
        .arg("/fo")
        .arg(&res_file)
        .arg(&rc_file)
        .current_dir(manifest.join("assets"))
        .status();

    match status {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg-bins={}", res_file.display()),
        other => println!(
            "cargo:warning=rc.exe failed ({other:?}); building without an embedded icon resource"
        ),
    }
}

fn find_rc() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("RC_EXE") {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }

    let roots = [
        std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".into()),
        std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into()),
    ];

    let mut best: Option<(Vec<u32>, PathBuf)> = None;
    for root in roots {
        let bins = Path::new(&root).join("Windows Kits").join("10").join("bin");
        let Ok(entries) = std::fs::read_dir(&bins) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let version: Vec<u32> = name.split('.').filter_map(|p| p.parse().ok()).collect();
            if version.len() < 3 {
                continue;
            }
            for arch in ["x64", "x86", "arm64"] {
                let candidate = entry.path().join(arch).join("rc.exe");
                if candidate.is_file() {
                    let better = best.as_ref().is_none_or(|(v, _)| *v < version);
                    if better {
                        best = Some((version.clone(), candidate));
                    }
                    break;
                }
            }
        }
    }
    best.map(|(_, p)| p)
}
