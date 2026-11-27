fn main() {
    // Only add drag plugin for desktop targets (not Android/iOS)
    if !cfg!(target_os = "android") && !cfg!(target_os = "ios") {
        println!("cargo:rustc-cfg=drag_plugin");
    }

    tauri_build::build()
}