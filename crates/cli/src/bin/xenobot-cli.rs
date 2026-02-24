//! Xenobot CLI binary entrypoint.

fn main() {
    if let Err(err) = xenobot_cli::app::run() {
        eprintln!("{}", err);
        std::process::exit(1);
    }
}
