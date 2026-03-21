use std::{env, fs, path::PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AbiManifest {
    symbol_count: u32,
    symbol_hash: String,
}

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set"));
    let manifest_path = manifest_dir.join("ffi-manifest.json");
    let build_profile = env::var("PROFILE").expect("PROFILE is not set");

    println!("cargo:rerun-if-changed={}", manifest_path.display());

    let manifest_raw = fs::read_to_string(&manifest_path).unwrap_or_else(|err| {
        panic!(
            "Failed to read {}: {err}. Run 'bun scripts/native-abi.ts --write' first.",
            manifest_path.display()
        )
    });
    let manifest: AbiManifest = serde_json::from_str(&manifest_raw)
        .unwrap_or_else(|err| panic!("Failed to parse {}: {err}", manifest_path.display()));

    println!(
        "cargo:rustc-env=OPENTUI_ABI_SYMBOL_COUNT={}",
        manifest.symbol_count
    );
    println!(
        "cargo:rustc-env=OPENTUI_ABI_SYMBOL_HASH={}",
        manifest.symbol_hash
    );
    println!("cargo:rustc-env=OPENTUI_BUILD_PROFILE={build_profile}");
}
