//! hive-host: on-box source of truth (sessions, transcripts, tmux, HTTP).

pub mod bind;
pub mod codex_bind;
pub mod discover;
pub mod fs;
pub mod http;
pub mod install;
pub mod labels;
pub mod meta;
pub mod run;
pub mod say;
pub mod sessions;
pub mod tmux;
pub mod transcript;

pub use hive_common::{HostConfig, HiveHome};

use anyhow::Result;
use clap::{Parser, Subcommand};
use hive_common::assert_bind_allowed;

#[derive(Parser, Debug)]
#[command(name = "hive-host", about = "Per-box hive daemon")]
pub struct Args {
    #[command(subcommand)]
    pub cmd: HostCmd,
}

#[derive(Subcommand, Debug)]
pub enum HostCmd {
    /// Run the HTTP daemon (foreground)
    Serve {
        #[arg(long)]
        bind: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        /// Allow 127.0.0.1 / ::1
        #[arg(long)]
        dev: bool,
        #[arg(long)]
        home: Option<String>,
    },
    /// Write systemd --user unit and enable it
    Install {
        #[arg(long)]
        bind: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        dev: bool,
        #[arg(long)]
        home: Option<String>,
        /// Absolute path to hive-host binary (default: this executable)
        #[arg(long)]
        bin: Option<String>,
        /// Copy ~/.cc/hosts and sessions into ~/.hive once
        #[arg(long)]
        migrate: bool,
    },
}

pub async fn run(args: Args) -> Result<()> {
    match args.cmd {
        HostCmd::Serve {
            bind,
            port,
            dev,
            home,
        } => {
            let home = HiveHome::resolve(home.as_deref())?;
            let cfg = HostConfig::load(&home)?;
            let bind = bind.unwrap_or(cfg.bind);
            let port = port.unwrap_or(cfg.port);
            assert_bind_allowed("hive-host", &bind, dev)?;
            http::serve(home, bind, port).await
        }
        HostCmd::Install {
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
            let cfg = HostConfig::load(&home)?;
            let bind = bind.unwrap_or(cfg.bind);
            let port = port.unwrap_or(cfg.port);
            let bin_path = bin.as_ref().map(std::path::PathBuf::from);
            let path = install::install(&home, &bind, port, dev, bin_path.as_deref())?;
            println!("wrote {}", path.display());
            Ok(())
        }
    }
}

/// Used by `hive host serve`.
pub async fn serve(
    bind: Option<String>,
    port: Option<u16>,
    dev: bool,
    home: Option<String>,
) -> Result<()> {
    run(Args {
        cmd: HostCmd::Serve {
            bind,
            port,
            dev,
            home,
        },
    })
    .await
}
