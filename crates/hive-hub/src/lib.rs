//! hive-hub: always-on fleet aggregator (poll hosts, proxy, SSE).

mod http;
mod install;
mod poll;
mod proxy;
mod state;

use anyhow::Result;
use clap::{Parser, Subcommand};
use hive_common::{assert_bind_allowed, HiveHome};

pub use hive_common::HubConfig;
pub use install::install;

#[derive(Parser, Debug)]
#[command(name = "hive-hub", about = "Always-on hive fleet aggregator")]
pub struct Args {
    #[command(subcommand)]
    pub cmd: HubCmd,
}

#[derive(Subcommand, Debug)]
pub enum HubCmd {
    Serve {
        #[arg(long)]
        bind: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        dev: bool,
        #[arg(long)]
        home: Option<String>,
    },
    Install {
        #[arg(long)]
        bind: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        dev: bool,
        #[arg(long)]
        home: Option<String>,
        #[arg(long)]
        bin: Option<String>,
        #[arg(long)]
        migrate: bool,
    },
}

pub async fn run(args: Args) -> Result<()> {
    match args.cmd {
        HubCmd::Serve {
            bind,
            port,
            dev,
            home,
        } => {
            let home = HiveHome::resolve(home.as_deref())?;
            let cfg = HubConfig::load(&home)?;
            let bind = bind.unwrap_or(cfg.bind.clone());
            let port = port.unwrap_or(cfg.port);
            assert_bind_allowed("hive-hub", &bind, dev)?;
            http::serve(bind, port, home, cfg).await
        }
        HubCmd::Install {
            bind,
            port,
            dev,
            home,
            bin,
            migrate,
        } => {
            let home = HiveHome::resolve(home.as_deref())?;
            if migrate {
                println!("{}", hive_common::migrate_from_cc(&home)?);
            }
            let cfg = HubConfig::load(&home)?;
            let bind = bind.unwrap_or(cfg.bind);
            let port = port.unwrap_or(cfg.port);
            let bin_path = bin.as_ref().map(std::path::PathBuf::from);
            let path = install::install(&home, &bind, port, dev, bin_path.as_deref())?;
            println!("wrote {}", path.display());
            Ok(())
        }
    }
}

/// Used by `hive hub serve`.
pub async fn serve(
    bind: Option<String>,
    port: Option<u16>,
    dev: bool,
    home: Option<String>,
) -> Result<()> {
    run(Args {
        cmd: HubCmd::Serve {
            bind,
            port,
            dev,
            home,
        },
    })
    .await
}
