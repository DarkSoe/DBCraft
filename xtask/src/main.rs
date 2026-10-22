//! Dev tasks for DBCraft.
//!
//! ```sh
//! cargo xtask layers
//! ```

use std::collections::BTreeMap;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("layers") => check_layers(),
        Some(other) => {
            eprintln!("unknown xtask: {other}");
            eprintln!("available: layers");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("usage: cargo xtask <layers>");
            ExitCode::FAILURE
        }
    }
}

fn dep_line_mentions(manifest: &str, crate_name: &str) -> bool {
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        // Match `name = { ... }` or `name.workspace = true` dependency keys.
        if let Some(rest) = trimmed.strip_prefix(crate_name)
            && (rest.starts_with(' ') || rest.starts_with('=') || rest.starts_with('.'))
        {
            return true;
        }
    }
    false
}

fn check_layers() -> ExitCode {
    // Allowed dependency layers: core < driver < engine < ui-egui < apps
    let layers: BTreeMap<&str, u8> = BTreeMap::from([
        ("dbcraft-core", 0),
        ("dbcraft-driver", 1),
        ("dbcraft-engine", 2),
        ("dbcraft-ui-egui", 3),
        ("dbcraft", 4),
        ("dbcraft-cli", 4),
    ]);

    let forbidden_ui = ["egui", "eframe", "rfd", "winit"];
    let mut ok = true;

    for (crate_name, layer) in &layers {
        if *layer >= 3 {
            continue;
        }
        let manifest = match *crate_name {
            "dbcraft-core" => "crates/core/Cargo.toml",
            "dbcraft-driver" => "crates/driver/Cargo.toml",
            "dbcraft-engine" => "crates/engine/Cargo.toml",
            _ => continue,
        };
        let Ok(text) = std::fs::read_to_string(manifest) else {
            eprintln!("missing {manifest}");
            ok = false;
            continue;
        };
        for dep in forbidden_ui {
            if dep_line_mentions(&text, dep) {
                eprintln!("{crate_name} (L{layer}) must not depend on {dep}");
                ok = false;
            }
        }
        for (other, other_layer) in &layers {
            if other == crate_name {
                continue;
            }
            if dep_line_mentions(&text, other) && other_layer > layer {
                eprintln!(
                    "{crate_name} (L{layer}) depends on higher layer {other} (L{other_layer})"
                );
                ok = false;
            }
        }
    }

    if ok {
        println!("layers ok");
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
