// Eject round-trips (roadmap Phase 4 exit): an ejected std recipe, unedited, renders the same
// pixels as the std recipe; an edit to it shows. Needs the CLI (`cargo build --release -p datars-cli`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync, readFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const cli = join(root, "target/release/datars");

const doc = (imports) => `
import { doc, data, group } from "@datars/sdk";
${imports}
export default doc({
  size: [480, 300],
  data: { v: data.values({ k: ["a", "b", "c"], n: [3, 1, 2] }, { key: "k" }) },
  scene: group({ key: "root", layout: { type: "stack", padding: 12 }, children: [plot({ data: "v", x: "k", y: "n", color: "k", children: [bar({ labels: true })] })] }),
});`;

test("an ejected recipe renders like the original, and edits to it show", { skip: !existsSync(cli) && "build the CLI first" }, () => {
  const dir = mkdtempSync(join(tmpdir(), "datars-eject-"));
  const hash = (file) => execFileSync(cli, ["render", file, "--dpr", "1", "--hash", "--out", join(dir, "x.png")], { encoding: "utf8" }).split("\n")[0].trim();
  writeFileSync(join(dir, "std.ts"), doc(`import { plot, bar } from "@datars/std";`));
  writeFileSync(join(dir, "ejected.ts"), doc(`import { plot } from "@datars/std";\nimport { bar } from "./recipes/bar";`));
  execFileSync(cli, ["eject", "std/bar", "--to", join(dir, "recipes")]);
  assert.equal(hash(join(dir, "ejected.ts")), hash(join(dir, "std.ts")), "unedited: identical pixels");
  const file = join(dir, "recipes/bar.ts");
  writeFileSync(file, readFileSync(file, "utf8").replace(`radius: t.number(0,`, `radius: t.number(8,`));
  assert.notEqual(hash(join(dir, "ejected.ts")), hash(join(dir, "std.ts")), "the edit shows");
});
