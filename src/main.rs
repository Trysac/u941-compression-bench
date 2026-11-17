fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    std::process::exit(compression_bench::cli::run(
        &args[1..],
        &mut stdout,
        &mut stderr,
    ));
}
