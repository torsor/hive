import { listRemoteDirs } from "./hub.js";

export function shellQuote(value) {
  return `'${String(value).replace(/'/g, "'\\''")}'`;
}

/** Remote-side quoting (matches hive-panel terminal.rs). */
function remoteShellQuote(value) {
  return `'${String(value).replace(/'/g, "'\"'\"'")}'`;
}

/** Match hive-panel `ssh_shell_at_argv` (Termius-friendly). */
export function sshShellAtArgv(host, dir) {
  const d = String(dir || "").trim();
  const remote = d
    ? `cd ${remoteShellQuote(d)} && exec \${SHELL:-/bin/zsh} -l`
    : "exec ${SHELL:-/bin/zsh} -l";
  return ["ssh", String(host || "").trim(), "-t", remote];
}

export function formatCommand(cmd) {
  if (Array.isArray(cmd)) {
    return cmd
      .map((p) => (/\s/.test(p) ? shellQuote(p) : String(p)))
      .join(" ");
  }
  return String(cmd);
}

export function sshCommandLine(argv) {
  if (
    argv[0] === "ssh" &&
    argv[2] === "-t" &&
    argv.length >= 4
  ) {
    return `ssh ${shellQuote(argv[1])} -t ${shellQuote(argv[3])}`;
  }
  return formatCommand(argv);
}

export function sshShellAtCommand(host, dir) {
  return sshCommandLine(sshShellAtArgv(host, dir));
}

export async function copyText(text, field) {
  if (navigator.clipboard && window.isSecureContext) {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch (_) {
      /* fall through */
    }
  }
  if (field) {
    field.focus();
    field.select();
    field.setSelectionRange(0, field.value.length);
    try {
      return document.execCommand("copy");
    } catch (_) {
      return false;
    }
  }
  return false;
}

export async function resolveRemotePath(host, path) {
  const h = String(host || "").trim();
  const p = String(path || "").trim();
  if (!h || !p) return p;
  try {
    const listing = await listRemoteDirs(h, p);
    return listing.path || p;
  } catch (_) {
    return p;
  }
}

export function appendPathCopyBlock(container, { host, dir, hint }) {
  const path = String(dir || "").trim();
  const h = String(host || "").trim();
  if (!path || !h) return;

  const block = document.createElement("div");
  block.className = "path-copy-block";

  const label = document.createElement("span");
  label.className = "path-copy-label";
  label.textContent = "Path on host";
  block.append(label);

  const display = document.createElement("div");
  display.className = "path-copy-display mono";
  display.textContent = path;
  display.title = path;
  block.append(display);

  const actions = document.createElement("div");
  actions.className = "path-copy-actions";

  const pathField = document.createElement("textarea");
  pathField.className = "path-copy-hidden";
  pathField.setAttribute("aria-hidden", "true");
  pathField.tabIndex = -1;
  pathField.value = path;
  block.append(pathField);

  const sshField = document.createElement("textarea");
  sshField.className = "path-copy-hidden";
  sshField.setAttribute("aria-hidden", "true");
  sshField.tabIndex = -1;
  const sshCmd = sshShellAtCommand(h, path);
  sshField.value = sshCmd;
  block.append(sshField);

  const result = document.createElement("span");
  result.className = "muted path-copy-result";

  const copyPath = document.createElement("button");
  copyPath.type = "button";
  copyPath.className = "btn btn--sm";
  copyPath.textContent = "Copy path";
  copyPath.onclick = async () => {
    const copied = await copyText(path, pathField);
    result.textContent = copied
      ? "Path copied."
      : "Path selected — use Copy from the menu.";
  };

  const copySsh = document.createElement("button");
  copySsh.type = "button";
  copySsh.className = "btn btn--sm";
  copySsh.textContent = "Copy SSH";
  copySsh.onclick = async () => {
    const copied = await copyText(sshCmd, sshField);
    result.textContent = copied
      ? "SSH command copied."
      : "Command selected — paste in Termius.";
  };

  actions.append(copyPath, copySsh, result);
  block.append(actions);

  if (hint) {
    const note = document.createElement("p");
    note.className = "muted path-copy-hint";
    note.textContent = hint;
    block.append(note);
  }

  container.append(block);
}
