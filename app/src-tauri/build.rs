//! App manifest (S11): only the commands of `ward_app_core::commands::COMMANDS` get a
//! permission (`allow-<command>`). With the manifest, a registered command that is not
//! on the list is denied; without it, Tauri would allow all of them.

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(ward_app_core::commands::COMMANDS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("tauri-build falhou (capabilities ou tauri.conf.json inválidos)");
}
