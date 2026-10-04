use hive_common::HiveHome;
use hive_local_agents::{
    agents_matching_tags, get, load, open_shell_line, register, remove, set, LocalAgent,
    LocalAgentInput, LocalAgentPatch,
};
use serde::Deserialize;

use crate::terminal::{self, TerminalMacos};

fn home() -> Result<HiveHome, String> {
    HiveHome::resolve(None).map_err(|e| e.to_string())
}

fn terminal_pref() -> TerminalMacos {
    let home = HiveHome::resolve(None).unwrap_or(HiveHome {
        root: std::path::PathBuf::from("/"),
    });
    let term = hive_common::ClientToml::load(&home).terminal;
    TerminalMacos::parse(&term)
}

#[tauri::command]
pub fn list_local_agents_cmd() -> Result<Vec<LocalAgent>, String> {
    let doc = load(&home()?).map_err(|e| e.to_string())?;
    Ok(doc.agents)
}

#[derive(Debug, Deserialize)]
pub struct LocalAgentRegisterArgs {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub resume: String,
    pub notes: Option<String>,
    pub agentmsg: Option<String>,
    pub provider: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[tauri::command]
pub fn register_local_agent_cmd(args: LocalAgentRegisterArgs) -> Result<LocalAgent, String> {
    register(
        &home()?,
        &args.id,
        LocalAgentInput {
            title: args.title,
            cwd: args.cwd,
            resume: args.resume,
            notes: args.notes,
            agentmsg: args.agentmsg,
            provider: args.provider,
            tags: args.tags,
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_local_agent_cmd(id: String) -> Result<(), String> {
    remove(&home()?, &id).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct LocalAgentSetArgs {
    pub id: String,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub resume: Option<String>,
    pub notes: Option<String>,
    pub agentmsg: Option<String>,
    pub provider: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[tauri::command]
pub fn set_local_agent_cmd(args: LocalAgentSetArgs) -> Result<LocalAgent, String> {
    set(
        &home()?,
        &args.id,
        LocalAgentPatch {
            title: args.title,
            cwd: args.cwd,
            resume: args.resume,
            notes: if args.notes.is_some() {
                Some(args.notes.clone())
            } else {
                None
            },
            agentmsg: if args.agentmsg.is_some() {
                Some(args.agentmsg.clone())
            } else {
                None
            },
            provider: if args.provider.is_some() {
                Some(args.provider.clone())
            } else {
                None
            },
            tags: args.tags,
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_local_agent_cmd(id: String) -> Result<String, String> {
    let agent = get(&home()?, &id).map_err(|e| e.to_string())?;
    let line = open_shell_line(&agent);
    terminal::open_zsh_command(&line, &agent.title, terminal_pref()).map_err(|e| e.to_string())?;
    Ok(format!("Opened {} in terminal", agent.id))
}

#[tauri::command]
pub fn open_local_group_cmd(
    tags: Vec<String>,
    match_all: bool,
) -> Result<String, String> {
    let doc = load(&home()?).map_err(|e| e.to_string())?;
    let normalized: Result<Vec<String>, _> = tags
        .iter()
        .map(|t| hive_local_agents::normalize_tag(t))
        .collect();
    let tags = normalized.map_err(|e| e.to_string())?;
    let agents = agents_matching_tags(&doc.agents, &tags, match_all);
    if agents.is_empty() {
        return Err(format!("no local agents match tags: {}", tags.join(", ")));
    }
    let count = agents.len();
    let term = terminal_pref();
    for a in agents {
        let line = open_shell_line(a);
        terminal::open_zsh_command(&line, &a.title, term).map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
    Ok(format!("Opened {count} local agent(s)"))
}
