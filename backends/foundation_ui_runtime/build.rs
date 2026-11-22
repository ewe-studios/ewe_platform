//! Select the actions that this output can execute.

use std::env;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    let explicit = ["BROWSER", "MOBILE", "SERVER"]
        .map(|role| env::var_os(format!("CARGO_FEATURE_ACTION_{role}")).is_some());
    let roles = if explicit.contains(&true) {
        explicit
    } else {
        let os = env::var("CARGO_CFG_TARGET_OS").expect("Cargo supplies the target OS");
        let families =
            env::var("CARGO_CFG_TARGET_FAMILY").expect("Cargo supplies the target family");
        match os.as_str() {
            "android" | "ios" => [false, true, false],
            _ if families.split(',').any(|family| family == "wasm") => [true, false, false],
            _ => [true, true, true],
        }
    };
    for (name, enabled) in ["browser", "mobile", "server", "host"].into_iter().zip([
        roles[0],
        roles[1],
        roles[2],
        roles[0] || roles[1],
    ]) {
        println!("cargo::rustc-check-cfg=cfg(ewe_action_{name})");
        if enabled {
            println!("cargo::rustc-cfg=ewe_action_{name}");
        }
    }
}
