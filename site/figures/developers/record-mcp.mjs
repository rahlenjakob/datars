// Records the developers page's MCP session: starts datars-mcp (the Model Context Protocol server,
// stdio), sends an agent's first three requests on the bench — initialize, tools/list, a lint of the
// flawed chart — and writes each request and reply to mcp-session.txt as they went over the wire.
// Long replies are cut (said where); nothing else is edited.
//
//   cargo build --release -p datars-mcp && node site/figures/developers/record-mcp.mjs
import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";

const root = resolve(dirname(new URL(import.meta.url).pathname), "../../..");
const server = spawn(join(root, "target/release/datars-mcp"), [], { cwd: root, stdio: ["pipe", "pipe", "inherit"] });
const replies = createInterface({ input: server.stdout });
const waiting = new Map();
replies.on("line", (line) => {
  const msg = JSON.parse(line);
  waiting.get(msg.id)?.(line);
});
let next = 1;
const log = [];
const cut = (s, n) => (s.length > n ? `${s.slice(0, n)} … [${s.length - n} more characters]` : s);
async function call(method, params, max = 900) {
  const req = JSON.stringify({ jsonrpc: "2.0", id: next++, method, ...(params ? { params } : {}) });
  const reply = new Promise((r) => waiting.set(JSON.parse(req).id, r));
  server.stdin.write(`${req}\n`);
  log.push(`→ ${req}`, `← ${cut(await reply, max)}`, "");
}
await call("initialize", { protocolVersion: "2024-11-05", capabilities: {}, clientInfo: { name: "record-mcp", version: "1" } });
await call("tools/list", undefined, 600);
await call("tools/call", { name: "lint", arguments: { doc: "site/figures/developers/bench-000.ts" } }, 2400);
server.stdin.end();
writeFileSync(join(root, "site/figures/developers/mcp-session.txt"), log.join("\n").trimEnd() + "\n");
console.log(log.join("\n"));
