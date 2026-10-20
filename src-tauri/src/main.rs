#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[cfg(all(target_os = "macos", debug_assertions))]
mod macos_dev;

fn main() {
    #[cfg(all(target_os = "macos", debug_assertions))]
    macos_dev::enter_bundle().expect("Could not prepare the macOS development app bundle");
    openstreamdeck_lib::run();
}
