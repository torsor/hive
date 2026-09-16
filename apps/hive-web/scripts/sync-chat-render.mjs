import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const src = join(root, "..", "hive-panel", "src", "chat-render.js");
const dest = join(root, "src", "chat-render.js");

mkdirSync(dirname(dest), { recursive: true });
copyFileSync(src, dest);
