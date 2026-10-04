//! `hive local` — laptop agent registry (~/.hive/local-agents.json).

use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use hive_common::HiveHome;
use hive_local_agents::{
    add, agents_matching_tags, get, load, normalize_tag, open_shell_line, register, remove, set,
    LocalAgent, LocalAgentInput, LocalAgentPatch,
};

use crate::local_launch;

pub const LOCAL_LONG_ABOUT: &str = "\
Bookmark agents on **this machine** (Cursor, Composer, local Claude) for hive-panel’s \
**Local** section. Not fleet tmux — use `hive run` for box sessions.

**Instantiate once (human):**

  hive local add --id my-agent --title \"Short label\" \\
    --cwd \"$PWD\" --resume 'cursor .' --tags laptop,my-team

**Self-registration (agent on each wake — upsert by id):**

  hive local register --id my-agent --title \"Short label\" \\
    --cwd \"$PWD\" --resume 'cursor .' \\
    --provider cursor --agentmsg agent:my-slug --tags laptop,my-team

Cursor: hive opens the **repo** (`cursor .`); you still pick the **chat/agent in the GUI**. \
Put which chat in --notes. When Cursor CLI resume exists, change --resume only.

**After laptop shutdown — restart a group:**

  hive local open-group --tag my-team --launch

Also: list, show, set, remove, open. Registry: ~/.hive/local-agents.json \
(override with --home / HIVE_HOME). See docs/superpowers/specs/2026-10-04-hive-local-agents-operator-guide.md.";

pub const LOCAL_AFTER_HELP: &str = "\
Examples:
  hive local list
  hive local show my-agent --json
  hive local open my-agent --launch
  hive local open-group --tag my-team --launch

Fleet sessions: hive run <host> <task> <dir>   Local bookmarks: hive local register …";

#[derive(Subcommand, Debug)]
pub enum LocalCmd {
    /// List registered local agents
    List {
        #[arg(long)]
        json: bool,
        #[arg(long = "tag", help = "Only agents with this tag (repeat for AND with --match-all)")]
        tag: Vec<String>,
        #[arg(long, help = "With multiple --tag, require all tags (default: any)")]
        match_all: bool,
    },
    /// Show one agent by id
    Show {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Create a new local agent (fails if id exists)
    Add {
        #[arg(long)]
        id: String,
        #[command(flatten)]
        fields: LocalAgentFields,
    },
    /// Create or update by id (for agent self-registration)
    Register {
        #[arg(long)]
        id: String,
        #[command(flatten)]
        fields: LocalAgentFields,
    },
    /// Update fields on an existing agent
    Set {
        id: String,
        #[command(flatten)]
        fields: LocalAgentFieldsOptional,
    },
    /// Remove an agent from the registry
    Remove { id: String },
    /// Print or run the shell line to resume (cd + command)
    Open {
        id: String,
        #[arg(long, help = "Open a new Terminal.app window on macOS")]
        launch: bool,
    },
    /// Open every agent matching tag(s) — restart a cohort after laptop shutdown
    #[command(
        about = "Restart all local agents that share a tag (e.g. my-team)",
        long_about = "Runs `open` for each agent whose tags match. \
        Default: any --tag matches. Use --match-all to require every tag. \
        --launch opens Terminal.app on macOS (one window per agent, ~400ms apart). \
        Without --launch, prints cd && resume lines for manual use."
    )]
    OpenGroup {
        #[arg(long = "tag", required = true, help = "Tag shared by the cohort (repeatable)")]
        tag: Vec<String>,
        #[arg(long, help = "Agent must have every --tag (default: any tag matches)")]
        match_all: bool,
        #[arg(long, help = "Launch Terminal.app per agent (macOS); else print commands only")]
        launch: bool,
    },
}

#[derive(Args, Clone, Debug)]
pub struct LocalAgentFields {
    #[arg(long, help = "Row label in the panel (1–120 chars)")]
    pub title: String,
    #[arg(long, help = "Absolute project directory (must exist); often \"$PWD\" from a register script")]
    pub cwd: String,
    #[arg(
        long,
        help = "Single shell command run after cd (how you resume this agent in a terminal)"
    )]
    pub resume: String,
    #[arg(long, help = "Optional notes")]
    pub notes: Option<String>,
    #[arg(long, help = "Optional agentmsg board slug, e.g. agent:my-agent")]
    pub agentmsg: Option<String>,
    #[arg(long, value_parser = clap::value_parser!(LocalProvider))]
    pub provider: Option<LocalProvider>,
    #[arg(
        long,
        value_delimiter = ',',
        help = "Tags for filtering groups (e.g. laptop, my-team — same rules as session tags)"
    )]
    pub tags: Vec<String>,
}

#[derive(Args, Clone, Debug)]
pub struct LocalAgentFieldsOptional {
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub cwd: Option<String>,
    #[arg(long)]
    pub resume: Option<String>,
    #[arg(long, help = "Omit unchanged; pass --notes '' to clear")]
    pub notes: Option<String>,
    #[arg(long, help = "Omit unchanged; pass --agentmsg '' to clear")]
    pub agentmsg: Option<String>,
    #[arg(long, value_parser = clap::value_parser!(LocalProvider), help = "Omit unchanged")]
    pub provider: Option<LocalProvider>,
    #[arg(long, value_delimiter = ',', help = "Replaces entire tag list when passed")]
    pub tags: Option<Vec<String>>,
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum LocalProvider {
    Cursor,
    Composer,
    Claude,
    Codex,
    Other,
}

impl LocalProvider {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Cursor => "cursor",
            Self::Composer => "composer",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Other => "other",
        }
    }
}

impl From<LocalAgentFields> for LocalAgentInput {
    fn from(f: LocalAgentFields) -> Self {
        LocalAgentInput {
            title: f.title,
            cwd: f.cwd,
            resume: f.resume,
            notes: f.notes,
            agentmsg: f.agentmsg,
            provider: f.provider.map(|p| p.as_str().into()),
            tags: f.tags,
        }
    }
}

pub fn run(home: &HiveHome, cmd: LocalCmd) -> Result<()> {
    match cmd {
        LocalCmd::List {
            json,
            tag,
            match_all,
        } => {
            let doc = load(home).map_err(map_err)?;
            let tags = normalize_filter_tags(&tag)?;
            let agents = agents_matching_tags(&doc.agents, &tags, match_all);
            if json {
                let filtered = LocalAgentDocFiltered {
                    version: doc.version,
                    agents: agents.into_iter().cloned().collect(),
                };
                println!("{}", serde_json::to_string_pretty(&filtered)?);
            } else if agents.is_empty() {
                println!("(no matching local agents)");
            } else {
                println!("{:<20}  {}  {}", "ID", "TITLE", "TAGS");
                for a in agents {
                    println!(
                        "{:<20}  {}  {}",
                        a.id,
                        a.title,
                        if a.tags.is_empty() {
                            "—".into()
                        } else {
                            a.tags.join(",")
                        }
                    );
                }
            }
        }
        LocalCmd::Show { id, json } => {
            let agent = get(home, &id).map_err(map_err)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&agent)?);
            } else {
                print_agent(&agent);
            }
        }
        LocalCmd::Add { id, fields } => {
            let agent = add(home, &id, fields.into()).map_err(map_err)?;
            eprintln!("added local agent {}", agent.id);
            print_agent(&agent);
        }
        LocalCmd::Register { id, fields } => {
            let agent = register(home, &id, fields.into()).map_err(map_err)?;
            eprintln!("registered local agent {}", agent.id);
            print_agent(&agent);
        }
        LocalCmd::Set { id, fields } => {
            let patch = patch_from_optional(fields)?;
            let agent = set(home, &id, patch).map_err(map_err)?;
            eprintln!("updated local agent {}", agent.id);
            print_agent(&agent);
        }
        LocalCmd::Remove { id } => {
            let agent = remove(home, &id).map_err(map_err)?;
            eprintln!("removed local agent {}", agent.id);
        }
        LocalCmd::Open { id, launch } => {
            let agent = get(home, &id).map_err(map_err)?;
            open_one(&agent, launch)?;
        }
        LocalCmd::OpenGroup {
            tag,
            match_all,
            launch,
        } => {
            let doc = load(home).map_err(map_err)?;
            let tags = normalize_filter_tags(&tag)?;
            let agents = agents_matching_tags(&doc.agents, &tags, match_all);
            if agents.is_empty() {
                bail!("no local agents match tags: {}", tags.join(", "));
            }
            eprintln!("opening {} agent(s)…", agents.len());
            for a in agents {
                open_one(a, launch)?;
                if launch {
                    std::thread::sleep(std::time::Duration::from_millis(400));
                }
            }
        }
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct LocalAgentDocFiltered {
    version: u32,
    agents: Vec<LocalAgent>,
}

fn open_one(agent: &LocalAgent, launch: bool) -> Result<()> {
    let line = open_shell_line(agent);
    if launch {
        local_launch::launch_shell_command(&agent.title, &line)?;
        eprintln!("{} ({})", agent.id, agent.title);
    } else {
        println!("# {} — {}", agent.id, agent.title);
        println!("{line}");
    }
    Ok(())
}

fn normalize_filter_tags(raw: &[String]) -> Result<Vec<String>> {
    raw.iter()
        .map(|t| normalize_tag(t).map_err(map_err))
        .collect()
}

fn patch_from_optional(f: LocalAgentFieldsOptional) -> Result<LocalAgentPatch> {
    let notes = f
        .notes
        .as_ref()
        .map(|s| optional_clear(s.clone()));
    let agentmsg = f
        .agentmsg
        .as_ref()
        .map(|s| optional_clear(s.clone()));
    let provider = f.provider.as_ref().map(|p| Some(p.as_str().into()));
    if f.title.is_none()
        && f.cwd.is_none()
        && f.resume.is_none()
        && notes.is_none()
        && agentmsg.is_none()
        && provider.is_none()
        && f.tags.is_none()
    {
        bail!("set: pass at least one field to update");
    }
    Ok(LocalAgentPatch {
        title: f.title,
        cwd: f.cwd,
        resume: f.resume,
        notes,
        agentmsg,
        provider,
        tags: f.tags,
    })
}

fn print_agent(a: &LocalAgent) {
    println!("id:         {}", a.id);
    println!("title:      {}", a.title);
    println!("cwd:        {}", a.cwd);
    println!("resume:     {}", a.resume);
    if let Some(n) = &a.notes {
        println!("notes:      {n}");
    }
    if let Some(m) = &a.agentmsg {
        println!("agentmsg:   {m}");
    }
    if let Some(p) = &a.provider {
        println!("provider:   {p}");
    }
    if !a.tags.is_empty() {
        println!("tags:       {}", a.tags.join(", "));
    }
    println!("updated_at: {}", a.updated_at);
    println!("open:       {}", open_shell_line(a));
}

fn map_err(e: hive_local_agents::Error) -> anyhow::Error {
    anyhow::Error::new(e).context("local agents registry")
}

fn optional_clear(s: String) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}
