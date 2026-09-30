# Pre-seed Claude Code's folder trust before a detached launch

**Date:** 2026-09-26
**Status:** deferred — do not implement
**Found during:** the research-room hand run on devbox (room-repo,
`docs/superpowers/notes/2026-09-22-hand-run-devbox.md`, step 7)

## Decision (2026-09-26)

Thrown out for now. The startup prompt is real, and this file edit is not
the fix.

`hasTrustDialogAccepted` is Claude's check that a directory may supply the
user's Claude process with project instructions, settings, and hooks. It
lives in `~/.claude.json` so the repo cannot answer it. hive-host writing
`true` answers it for the user, and the answer stays after the session ends.

`spawn` would do that for whatever `dir` the client sends. The host
canonicalizes the path and has no directory allowlist; Tailscale membership
is the only gate. The trust key is the git root, so one subdirectory marks
the whole repository trusted for later Claude runs, including ones hive did
not start. A careful rewrite of the file protects the rest of
`~/.claude.json`. It does not make the grant safe.

Leave the prompt in place. A work agent should not be given this
user-level file as context.

## Problem

`hive run <host> <task> <dir>` with the `claude` provider starts Claude Code
in a tmux session in a directory it has never seen. Claude Code then shows
its per-folder trust prompt ("Is this a project you created or one you
trust?") with **"No, exit" as the default**. A detached session sits on
that prompt until someone attaches, and an automated Enter kills it. On
2026-09-22 the workaround was:

    ssh devbox 'tmux send-keys -t hive-example-room Down "" && tmux send-keys -t hive-example-room Enter'

Every fresh room hits this; it is the one step in a room's start-up that
still needs a human at the keyboard.

## Precedent in hive-host

The Codex provider already solves the same problem for Codex:
`crates/hive-host/src/run.rs::codex_projects_override` passes
`-c projects={"<dir>"={trust_level="trusted"}}` on the command line, and
`codex_bind.rs:105` watches the pane for the trust text as a fallback.
Claude Code needs the equivalent, but it has no flag.

## The documented mechanism

Per Claude Code's permissions documentation ("What runs before you trust a
folder"), there is no CLI flag, environment variable, or settings key that
pre-accepts the prompt. The trust record lives in **`~/.claude.json`**:

```json
{
  "projects": {
    "/home/user/projects/example/example": {
      "hasTrustDialogAccepted": true
    }
  }
}
```

The key is the **git repository root** of the working directory, or the
directory itself when it is not inside a repository. `claude -p` never
shows the dialog; `claude --bg` refuses to start ("Workspace not trusted")
rather than prompt. Interactive sessions, which is what hive starts, prompt.

## Requested change

Before spawning a `claude` provider session in `run.rs::spawn`:

1. Resolve the trust key: `git -C <dir> rev-parse --show-toplevel`, falling
   back to the canonicalized `<dir>` when that fails.
2. Read `~/.claude.json` on the host (the user running hive-host). Absent
   file → start from `{}`. Unparseable file → refuse to spawn with a clear
   error rather than overwrite it; the file holds other Claude Code state.
3. Set `projects[<key>].hasTrustDialogAccepted = true`, creating the
   `projects` map and the entry as needed and touching nothing else in the
   entry or the file.
4. Write atomically (temp file in `~/`, rename over), preserving mode.
5. Log one line: `trust: seeded <key>` or `trust: already <key>`.

Do the same in `resume` when the recorded dir is not yet trusted (a record
can be lost if `~/.claude.json` is recreated).

Not requested: a pane watcher like `codex_bind`'s. If the seed works there
is nothing to watch for; if the file format changes, a watcher that types
Down+Enter would be guessing at UI. Fail visibly instead: if the pane shows
the trust text within the first seconds after launch, record it in the
session metadata as `trust_prompt: true` so `hive` can report it.

## Tests

- Fresh `~/.claude.json` absent: spawn creates it with exactly the one
  entry, mode 0600.
- Existing file with other projects and top-level keys: the entry is added;
  every other byte of meaning survives (compare parsed JSON minus the new
  entry).
- Dir inside a repo: key is the repo root, not the subdirectory.
- Entry already true: no write (mtime unchanged), log says `already`.
- Corrupt file: spawn refused, file untouched.

## Note

The root cause belongs to Claude Code, which has no non-interactive way to
grant trust. If a flag appears (`--trust-workspace` or similar), prefer it
and delete the file edit.
