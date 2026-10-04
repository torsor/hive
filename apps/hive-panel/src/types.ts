/** JSON shapes shared with `hive-protocol` (Rust). Keep field names in sync. */

export type SessionRow = {
  host?: string | null;
  task: string;
  state: string;
  started?: string;
  dir?: string;
  last?: string | null;
  provider?: string;
  starred?: boolean;
  tags?: string[];
  tmux?: string | null;
};

export type HostFleet = {
  host: string;
  error?: string | null;
  sessions: SessionRow[];
};

export type Fleet = {
  hosts: HostFleet[];
  generated_at: string;
};

export type ChatBlock = {
  kind: string;
  role: string;
  text: string;
  ts?: string | null;
  id?: string | null;
  failed?: boolean;
  /** Local-only optimistic send state (never from host transcript). */
  pending?: "queued" | "sending" | "failed";
  pendingError?: string;
};

/** @deprecated Use ChatBlock — pending lives on the block during outbound queue drain. */
export type ChatBlockUi = ChatBlock;

export type TranscriptCursor = {
  session_id?: string | null;
  offset: number;
};

export type BindingCandidate = {
  session_id: string;
  thread_name?: string | null;
  updated_at?: string | null;
  current: boolean;
  live: boolean;
  suggested: boolean;
  warning?: string | null;
};

export type BindingsDoc = {
  bound?: string | null;
  pane_session_id?: string | null;
  suggested?: string | null;
  candidates: BindingCandidate[];
};

export type TranscriptDoc = {
  session_id?: string | null;
  reset: boolean;
  blocked_on_prompt: boolean;
  cursor: TranscriptCursor;
  blocks: ChatBlock[];
  has_earlier?: boolean;
  earlier_until?: number | null;
};

export type FsEntry = {
  name: string;
  is_dir: boolean;
};

export type DirListing = {
  path: string;
  entries: FsEntry[];
};

export type HubEvent = {
  kind: string;
  host?: string | null;
  task?: string | null;
};

/** Flattened table row (fleet + UI flags). */
export type FlatSessionRow = {
  host: string;
  state: string;
  task: string;
  provider: string;
  dir: string;
  started: string;
  last: string | null;
  error: string | null;
  actionable: boolean;
  starred: boolean;
  tags: string[];
};

export type ConfigView = {
  hive_home: string;
  hub: string;
  terminal: string;
};

export type Theme =
  | "thing"
  | "thing-light"
  | "torsor"
  | "torsor-dark"
  | "torsor-b"
  | "torsor-b-dark";

/** ~/.hive/local-agents.json row (keep in sync with hive-local-agents). */
export type LocalAgent = {
  id: string;
  title: string;
  cwd: string;
  resume: string;
  notes?: string | null;
  agentmsg?: string | null;
  provider?: string | null;
  tags?: string[];
  updated_at: string;
};

export type Filters = {
  states: string[];
  hosts: string[];
  providers: string[];
  tags: string[];
  starredOnly: boolean;
  showHidden: boolean;
};
