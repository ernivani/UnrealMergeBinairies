use unreal_merge::app_mode::{AppMode, parse_argv};

#[test]
fn cli_subcommand_argv_routes_to_cli() {
    let argv = vec![
        "unreal-merge".to_string(),
        "scan".to_string(),
    ];
    match parse_argv(&argv) {
        AppMode::Cli => {}
        other => panic!("expected Cli, got {:?}", other),
    }
}

#[test]
fn no_args_routes_to_standalone_gui() {
    let argv = vec!["unreal-merge".to_string()];
    match parse_argv(&argv) {
        AppMode::StandaloneGui => {}
        other => panic!("expected StandaloneGui, got {:?}", other),
    }
}

#[test]
fn git_driver_argv_routes_to_git_driver_gui() {
    let argv = vec![
        "unreal-merge".to_string(),
        "--git-driver".to_string(),
        "anc".to_string(),
        "ours.tmp".to_string(),
        "theirs.tmp".to_string(),
        "a.uasset".to_string(),
    ];
    match parse_argv(&argv) {
        AppMode::GitDriverGui {
            ancestor,
            ours,
            theirs,
            path,
        } => {
            assert_eq!(ancestor, "anc");
            assert_eq!(ours, "ours.tmp");
            assert_eq!(theirs, "theirs.tmp");
            assert_eq!(path, "a.uasset");
        }
        other => panic!("expected GitDriverGui, got {:?}", other),
    }
}
