use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "math-signal-core-server",
    version,
    about = "Thin HTTP API adapter for math-signal-core"
)]
struct Args {
    /// Address to bind, for example 127.0.0.1:3000.
    #[arg(long, default_value = "127.0.0.1:3000")]
    addr: String,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    eprintln!("math-signal-core-server listening on http://{}", args.addr);
    math_signal_core_server::serve(&args.addr)
}
