//! `hive` CLI — fleet commands plus thin wrappers around hive-host and hive-hub.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use hive_client::Client;
use hive_common::HiveHome;
use hive_host::{Args as HostArgs, HostCmd};
use hive_hub::{Args as HubArgs, HubCmd};
use hive_protocol::{BindingsDoc, Fleet};

#[derive(Parser, Debug)]
#[command(name = "hive", about = "Hive fleet CLI — talks to hive-hub or local hive-host")]
pub struct Cli {
    #[arg(long, global = true)]
    pub home: Option<String>,
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    Status,
    Attach {
        host: String,
        task: String,
    },
    Say {
        host: String,
        task: String,
        #[arg(long)]
        text: String,
    },
    Stop {
        host: String,
        task: String,
    },
    Restart {
        host: String,
        task: String,
    },
    Kill {
        host: String,
        task: String,
    },
    Label {
        host: String,
        task: String,
        op: String,
    },
    Run {
        host: String,
        task: String,
        dir: String,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        resume: bool,
        extra: Vec<String>,
    },
    Transcript {
        host: String,
        task: String,
        #[arg(long)]
        tail: Option<u32>,
        #[arg(long)]
        after: Option<String>,
    },
    Bind {
        host: String,
        task: String,
        /// Persist this session id as the task's transcript binding
        #[arg(long)]
        to: Option<String>,
    },
    Fs {
        host: String,
        #[arg(long)]
        path: Option<String>,
    },
    Panel,
    Host {
        #[command(subcommand)]
        cmd: DaemonCmd,
    },
    Hub {
        #[command(subcommand)]
        cmd: DaemonCmd,
    },
}

#[derive(Subcommand, Debug)]
pub enum DaemonCmd {
    Serve {
        #[arg(long)]
        bind: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        dev: bool,
    },
    Install {
        #[arg(long)]
        bind: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        dev: bool,
        /// Copy ~/.cc/hosts and sessions into ~/.hive once
        #[arg(long)]
        migrate: bool,
    },
}

pub fn print_bindings(host: &str, task: &str, doc: &BindingsDoc) {
    println!(
        "{host}\t{task}\tbound={}\tsuggested={}\tpane={}",
        doc.bound.as_deref().unwrap_or("-"),
        doc.suggested.as_deref().unwrap_or("-"),
        doc.pane_session_id.as_deref().unwrap_or("-"),
    );
    for c in &doc.candidates {
        let mark = if c.suggested {
            "*"
        } else if c.current {
            "="
        } else {
            " "
        };
        let mut flags = Vec::new();
        if c.current {
            flags.push("current");
        }
        if c.live {
            flags.push("live");
        }
        if c.suggested {
            flags.push("suggested");
        }
        if let Some(w) = &c.warning {
            flags.push(w.as_str());
        }
        println!(
            "{mark} {}\t{}\t{}\t{}",
            c.session_id,
            c.thread_name.as_deref().unwrap_or("-"),
            c.updated_at.as_deref().unwrap_or("-"),
            flags.join(" ")
        );
    }
}

pub fn print_status(fleet: &Fleet) {
    for host in &fleet.hosts {
        if let Some(err) = &host.error {
            println!("{}\t—\t(unreachable)\t{err}", host.host);
            continue;
        }
        if host.sessions.is_empty() {
            println!("{}\t—\t(no sessions)", host.host);
            continue;
        }
        for s in &host.sessions {
            let star = if s.starred { "*" } else { " " };
            println!(
                "{star}{}\t{}\t{}\t{}\t{}",
                host.host,
                s.task,
                s.state,
                s.provider,
                s.dir
            );
            if let Some(last) = &s.last {
                println!("    {last}");
            }
        }
    }
}

pub async fn run(cli: Cli) -> Result<()> {
    let home = HiveHome::resolve(cli.home.as_deref())?;
    match cli.cmd {
        Cmd::Host { cmd } => daemon_host(cmd, &home).await,
        Cmd::Hub { cmd } => daemon_hub(cmd, &home).await,
        Cmd::Panel => {
            let bin = which_panel()?;
            let st = Command::new(bin).status()?;
            if st.success() {
                Ok(())
            } else {
                bail!("hive-panel exited {st}")
            }
        }
        other => client_cmd(other, &home).await,
    }
}

fn which_panel() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("HIVE_PANEL") {
        return Ok(PathBuf::from(p));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let cand = dir.join("hive-panel");
            if cand.exists() {
                return Ok(cand);
            }
        }
    }
    Ok(PathBuf::from("hive-panel"))
}

pub fn which_bin(name: &str) -> Option<PathBuf> {
    let out = Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {name}"))
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if p.is_empty() {
        None
    } else {
        Some(PathBuf::from(p))
    }
}

async fn daemon_host(cmd: DaemonCmd, home: &HiveHome) -> Result<()> {
    let home_s = Some(home.root.display().to_string());
    match cmd {
        DaemonCmd::Serve { bind, port, dev } => hive_host::serve(bind, port, dev, home_s).await,
        DaemonCmd::Install {
            bind,
            port,
            dev,
            migrate,
        } => {
            hive_host::run(HostArgs {
                cmd: HostCmd::Install {
                    bind,
                    port,
                    dev,
                    home: home_s,
                    bin: which_bin("hive-host").map(|p| p.display().to_string()),
                    migrate,
                },
            })
            .await
        }
    }
}

async fn daemon_hub(cmd: DaemonCmd, home: &HiveHome) -> Result<()> {
    let home_s = Some(home.root.display().to_string());
    match cmd {
        DaemonCmd::Serve { bind, port, dev } => hive_hub::serve(bind, port, dev, home_s).await,
        DaemonCmd::Install {
            bind,
            port,
            dev,
            migrate,
        } => {
            hive_hub::run(HubArgs {
                cmd: HubCmd::Install {
                    bind,
                    port,
                    dev,
                    home: home_s,
                    bin: which_bin("hive-hub").map(|p| p.display().to_string()),
                    migrate,
                },
            })
            .await
        }
    }
}

async fn client_cmd(cmd: Cmd, home: &HiveHome) -> Result<()> {
    let client = Client::new(home)?;
    match cmd {
        Cmd::Status => {
            let fleet = client.fetch_fleet().await?;
            print_status(&fleet);
        }
        Cmd::Attach { host, task } => {
            let att = client.attach(&host, &task).await?;
            let st = Command::new(&att.command[0])
                .args(&att.command[1..])
                .status()?;
            if !st.success() {
                bail!("attach failed: {st}");
            }
        }
        Cmd::Say { host, task, text } => {
            let out = client.say(&host, &task, &text).await?;
            println!("{}", out.output);
        }
        Cmd::Stop { host, task } => {
            let out = client.session_action(&host, &task, "stop").await?;
            println!("{}", out.output);
        }
        Cmd::Restart { host, task } => {
            let out = client.session_action(&host, &task, "restart").await?;
            println!("{}", out.output);
        }
        Cmd::Kill { host, task } => {
            let out = client.session_action(&host, &task, "kill").await?;
            println!("{}", out.output);
        }
        Cmd::Label { host, task, op } => {
            let out = client.label(&host, &task, &op).await?;
            println!("{}", out.output);
        }
        Cmd::Run {
            host,
            task,
            dir,
            provider,
            resume,
            extra,
        } => {
            let out = client
                .spawn(
                    &host,
                    &hive_protocol::SpawnRequest {
                        task,
                        dir: Some(dir),
                        provider,
                        extra_args: extra,
                        auto: false,
                        resume,
                    },
                )
                .await?;
            println!("{}", out.output);
        }
        Cmd::Transcript {
            host,
            task,
            tail,
            after,
        } => {
            let doc = client
                .transcript(&host, &task, after.as_deref(), tail, None)
                .await?;
            println!("{}", serde_json::to_string_pretty(&doc)?);
        }
        Cmd::Bind { host, task, to } => {
            let doc = if let Some(session_id) = to {
                client.bind(&host, &task, &session_id).await?
            } else {
                client.bindings(&host, &task).await?
            };
            print_bindings(&host, &task, &doc);
        }
        Cmd::Fs { host, path } => {
            let listing = client.fs(&host, path.as_deref()).await?;
            println!("{}", serde_json::to_string_pretty(&listing)?);
        }
        _ => unreachable!(),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_subcommand_migrate_defaults_false() {
        use clap::Parser;
        let cli = Cli::parse_from(["hive", "host", "install"]);
        match cli.cmd {
            Cmd::Host {
                cmd: DaemonCmd::Install { migrate, .. },
            } => assert!(!migrate),
            _ => panic!("expected host install"),
        }
    }

    #[test]
    fn bind_subcommand_lists_without_to() {
        use clap::Parser;
        let cli = Cli::parse_from(["hive", "bind", "hub", "period-index"]);
        match cli.cmd {
            Cmd::Bind { host, task, to } => {
                assert_eq!(host, "hub");
                assert_eq!(task, "period-index");
                assert_eq!(to, None);
            }
            _ => panic!("expected bind"),
        }
    }

    #[test]
    fn print_bindings_marks_suggested() {
        let doc = BindingsDoc {
            bound: Some("aaa".into()),
            pane_session_id: Some("bbb".into()),
            suggested: Some("bbb".into()),
            candidates: vec![hive_protocol::BindingCandidate {
                session_id: "bbb".into(),
                thread_name: Some("period-index".into()),
                updated_at: Some("2026-08-15T20:24:08Z".into()),
                current: false,
                live: true,
                suggested: true,
                warning: None,
            }],
        };
        print_bindings("hub", "period-index", &doc);
    }
}
