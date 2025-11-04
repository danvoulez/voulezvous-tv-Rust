use clap::Parser;

fn main() {
    let cli = vvtvctl::Cli::parse();
    if let Err(err) = vvtvctl::run(cli) {
        eprintln!("erro: {err}");
        std::process::exit(1);
    }
}
