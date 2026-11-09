//! Manifesto do app (S11): só os comandos de `ward_app_nucleo::comandos::COMANDOS`
//! ganham permissão (`allow-<comando>`). Com o manifesto, comando registrado que não
//! está na lista é negado; sem ele, o Tauri liberaria todos.

fn main() {
    let manifesto = tauri_build::AppManifest::new().commands(ward_app_nucleo::comandos::COMANDOS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifesto))
        .expect("tauri-build falhou (capabilities ou tauri.conf.json inválidos)");
}
