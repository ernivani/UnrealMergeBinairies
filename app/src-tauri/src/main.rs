//! Entry point. Peeks argv via app_mode::parse_argv and either:
//!  - hands off to the Plan 2 CLI dispatch (Cli mode, or git-driver mode when
//!    UNREAL_MERGE_RESOLUTION is set for headless / CI scenarios), or
//!  - boots Tauri with the AppMode inserted into managed state (GUI mode).

use unreal_merge::app_mode::{AppMode, parse_argv};

fn run_cli() {
    if let Err(e) = unreal_merge::cli::run() {
        eprintln!("error: {:#}", e);
        std::process::exit(1);
    }
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let mode = parse_argv(&argv);

    // UNREAL_MERGE_RESOLUTION keeps Plan 2's headless env-var-driven path alive
    // even after Plan 3 introduced the GUI. Useful for CI smoke tests and for
    // scripting (`UNREAL_MERGE_RESOLUTION=theirs git merge ...`) where popping a
    // window every conflict is unacceptable. If the env var is set, route
    // git-driver mode through the CLI dispatch instead of opening the GUI.
    let headless_override = std::env::var("UNREAL_MERGE_RESOLUTION").is_ok();

    match mode {
        AppMode::Cli => run_cli(),
        AppMode::GitDriverGui { .. } if headless_override => run_cli(),
        other => {
            tauri::Builder::default()
                .manage(other)
                .invoke_handler(tauri::generate_handler![
                    unreal_merge::ipc::get_app_mode,
                    unreal_merge::ipc::diff_snapshots,
                    unreal_merge::ipc::diff_graphs,
                    unreal_merge::ipc::diff_graphs_three_way,
                    unreal_merge::ipc::apply_resolution,
                    unreal_merge::ipc::apply_graph_merge,
                    unreal_merge::ipc::export_asset,
                    unreal_merge::ipc::close_with_exit,
                ])
                .run(tauri::generate_context!())
                .expect("error while running tauri application");
        }
    }
}
