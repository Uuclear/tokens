fn main() {
    if let Err(e) = tokens::cli::run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
