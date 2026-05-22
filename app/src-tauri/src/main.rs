fn main() {
    if let Err(e) = unreal_merge::cli::run() {
        eprintln!("error: {:#}", e);
        std::process::exit(1);
    }
}
