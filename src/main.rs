//! HJSON Editor CLI

mod cli;

fn main() {
    if let Err(error) = cli::run() {
        // anyhow の連鎖（原因）まで表示する
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
