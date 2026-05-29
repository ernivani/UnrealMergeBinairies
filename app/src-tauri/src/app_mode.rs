//! How the binary should behave for a given invocation. The same `unreal-merge.exe`
//! is both a Plan 2 CLI (no GUI) and a Plan 3 Tauri app - argv decides which.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AppMode {
    /// Plan 2 CLI subcommands - no GUI, hand off to `cli::run`.
    Cli,
    /// No args - open the GUI in standalone mode (scan current dir for conflicts).
    StandaloneGui,
    /// Git invoked us as a merge driver with 4 positional args.
    GitDriverGui {
        ancestor: String,
        ours: String,
        theirs: String,
        path: String,
    },
}

/// Parse `std::env::args().collect::<Vec<_>>()` into an `AppMode`.
pub fn parse_argv(argv: &[String]) -> AppMode {
    if argv.len() <= 1 {
        return AppMode::StandaloneGui;
    }

    // --git-driver mode: exactly 4 positional args after the flag.
    if let Some(pos) = argv.iter().position(|a| a == "--git-driver") {
        let rest = &argv[pos + 1..];
        if rest.len() >= 4 {
            return AppMode::GitDriverGui {
                ancestor: rest[0].clone(),
                ours: rest[1].clone(),
                theirs: rest[2].clone(),
                path: rest[3].clone(),
            };
        }
        // Wrong arity - fall through to CLI so clap produces a real error.
    }

    // Any other argv shape (install/uninstall/scan/export/diff/--help/--version)
    // routes to the CLI.
    AppMode::Cli
}
