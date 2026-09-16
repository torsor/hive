use clap::Parser;
use hive_host::{run, Args};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run(Args::parse()).await
}
