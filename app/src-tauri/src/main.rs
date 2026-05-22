//! Entry point. Peeks argv via app_mode::parse_argv and either:
//!  - hands off to the Plan 2 CLI dispatch (Cli mode), or
//!  - boots Tauri with the AppMode inserted into managed state.

use unreal_merge::app_mode::{AppMode, parse_argv};

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let mode = parse_argv(&argv);

    match mode {
        AppMode::Cli => {
            if let Err(e) = unreal_merge::cli::run() {
                eprintln!("error: {:#}", e);
                std::process::exit(1);
            }
        }
        other => {
            tauri::Builder::default()
                .manage(other)
                .invoke_handler(tauri::generate_handler![
                    unreal_merge::ipc::get_app_mode,
                    unreal_merge::ipc::diff_snapshots,
                    unreal_merge::ipc::apply_resolution,
                    unreal_merge::ipc::export_asset,
                    unreal_merge::ipc::close_with_exit,
                ])
                .run(tauri::generate_context!())
                .expect("error while running tauri application");
        }
    }
}
