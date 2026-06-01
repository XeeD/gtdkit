use clap::Parser;
use miette::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_target(false)
        .compact()
        .init();

    let cli = gtdkit::cli::Cli::parse();
    if let Err(err) = gtdkit::run(cli).await {
        anstream::eprintln!("{err:?}");
        std::process::exit(1);
    }
    Ok(())
}
