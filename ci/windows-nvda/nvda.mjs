// What NVDA says over the web element (Windows; see nvda.yml): serves the pages in <dir>, opens
// each in Edge, moves NVDA through it item by item (its browse-mode "next") for 25 items, then
// a few keys on the controls, and writes each step and what NVDA spoke to <out>/<chart>.txt.
//   node ci/windows-nvda/nvda.mjs <pages dir> <out dir>
import { createServer } from "node:http";
import { readFileSync, existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join, extname, normalize } from "node:path";
import { execSync } from "node:child_process";
import { nvda } from "@guidepup/guidepup";

const [dir, out] = process.argv.slice(2);
mkdirSync(out, { recursive: true });
const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json", ".ttf": "font/ttf", ".woff2": "font/woff2" };
const server = createServer((req, res) => {
  const file = normalize(join(dir, decodeURIComponent(new URL(req.url, "http://x").pathname)));
  if (!file.startsWith(normalize(dir)) || !existsSync(file)) return void res.writeHead(404).end();
  res.writeHead(200, { "Content-Type": types[extname(file)] ?? "application/octet-stream" });
  res.end(readFileSync(file));
});
await new Promise((r) => server.listen(8800, "127.0.0.1", r));
await nvda.start();
try {
  for (const chart of ["votes", "budget", "controls", "prices"]) {
    execSync(`start msedge --new-window http://127.0.0.1:8800/${chart}.html`, { shell: "cmd.exe" });
    await new Promise((r) => setTimeout(r, 8000));
    const lines = [];
    const say = async (step) => { lines.push(`[${step}] ${await nvda.lastSpokenPhrase()}`); };
    await say("open");
    for (let i = 0; i < 25; i++) { await nvda.next(); await say("next"); }
    writeFileSync(join(out, `${chart}.txt`), lines.join("\n") + "\n");
    writeFileSync(join(out, `${chart}.log.txt`), (await nvda.spokenPhraseLog()).join("\n") + "\n");
    await nvda.clearSpokenPhraseLog?.();
    execSync("taskkill /IM msedge.exe /F", { shell: "cmd.exe" });
  }
} finally {
  await nvda.stop();
  server.close();
}
