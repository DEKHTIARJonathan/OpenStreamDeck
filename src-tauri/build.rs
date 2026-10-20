fn main() {
    // The dev executable embeds this metadata; rebuild when its display name changes.
    println!("cargo:rerun-if-changed=Info.plist");
    // Clear the CLI-injected environment override so the explicit runtime
    // policy below is authoritative on every platform.
    // This runs single-threaded, before tauri-build starts work.
    std::env::remove_var("STATIC_VCRUNTIME");
    let attributes = tauri_build::Attributes::new()
        .windows_attributes(tauri_build::WindowsAttributes::new().static_vc_runtime(true));
    tauri_build::try_build(attributes).expect("Tauri build configuration failed");
}
