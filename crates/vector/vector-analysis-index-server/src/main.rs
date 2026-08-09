use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "vector-analysis-index-server",
    version,
    about = "Thin HTTP API adapter for vector-analysis-index"
)]
struct Args {
    /// Address to bind, for example 127.0.0.1:3000.
    #[arg(long, default_value = "127.0.0.1:3000")]
    addr: String,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    eprintln!(
        "vector-analysis-index-server listening on http://{}",
        args.addr
    );
    vector_analysis_index_server::serve(&args.addr)
}
