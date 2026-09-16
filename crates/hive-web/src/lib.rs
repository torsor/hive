//! hive-web: Tailscale-bound static server for the phone SPA.

mod http;
mod install;

use anyhow::Result;
use clap::{Parser, Subcommand};
use hive_common::{assert_bind_allowed, ensure_dir, HiveHome, WebConfig};

pub use install::install;

#[derive(Parser, Debug)]
#[command(name = "hive-web", about = "Phone console static server")]
pub struct Args {
    #[command(subcommand)]
    pub cmd: WebCmd,
}

#[derive(Subcommand, Debug)]
pub enum WebCmd {
    Serve {
        #[arg(long)]
        bind: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        root: Option<String>,
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
        root: Option<String>,
        #[arg(long)]
        dev: bool,
        #[arg(long)]
        home: Option<String>,
        #[arg(long)]
        bin: Option<String>,
    },
}

pub async fn run(args: Args) -> Result<()> {
    match args.cmd {
        WebCmd::Serve {
            bind,
            port,
            root,
            dev,
            home,
        } => {
            let home = HiveHome::resolve(home.as_deref())?;
            let cfg = WebConfig::load(&home)?;
            let bind = bind.unwrap_or(cfg.bind.clone());
            let port = port.unwrap_or(cfg.port);
            let root = root.unwrap_or(cfg.root.clone());
            assert_bind_allowed("hive-web", &bind, dev)?;
            ensure_dir(std::path::Path::new(&root))?;
            http::serve(bind, port, root).await
        }
        WebCmd::Install {
            bind,
            port,
            root,
            dev,
            home,
            bin,
        } => {
            let home = HiveHome::resolve(home.as_deref())?;
            ensure_dir(&home.root)?;
            let mut cfg = WebConfig::load(&home)?;
            if let Some(r) = root {
                cfg.root = r;
            }
            cfg.bind = bind.unwrap_or(cfg.bind);
            cfg.port = port.unwrap_or(cfg.port);
            cfg.save(&home)?;
            let bin_path = bin.as_ref().map(std::path::PathBuf::from);
            let path = install::install(&home, &cfg.bind, cfg.port, &cfg.root, dev, bin_path.as_deref())?;
            println!("wrote {}", path.display());
            Ok(())
        }
    }
}
