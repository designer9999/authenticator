fn main() {
    println!("cargo:rerun-if-env-changed=TAURI_UPDATER_PUBKEY");
    println!("cargo:rerun-if-env-changed=TAURI_UPDATER_ENDPOINT");

    if let Ok(pubkey) = std::env::var("TAURI_UPDATER_PUBKEY") {
        let normalized = pubkey.replace("\r\n", "\n").replace('\n', "\\n");
        println!("cargo:rustc-env=TAURI_UPDATER_PUBKEY={normalized}");
    }

    if let Ok(endpoint) = std::env::var("TAURI_UPDATER_ENDPOINT") {
        println!("cargo:rustc-env=TAURI_UPDATER_ENDPOINT={endpoint}");
    }

    tauri_build::build()
}
