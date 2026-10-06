// Compile one TypeScript document (using @datars/sdk and @datars/std) and print its JSON IR.
//   node scripts/doc-to-json.mjs path/to/doc.ts
import { build } from "esbuild";
import { mkdtempSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { embedLocalRecipes } from "./local-recipes.mjs";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";

const root = resolve(new URL("..", import.meta.url).pathname);
const src = resolve(process.argv[2]);
const dir = mkdtempSync(join(tmpdir(), "datars-doc-"));
const out = join(dir, "doc.mjs");
try {
  // A syntax or import error: esbuild has printed it, with the line and a caret. Nothing to add.
  await build({ entryPoints: [src], bundle: true, format: "esm", platform: "node", outfile: out, logLevel: "error",
    alias: { "@datars/sdk": join(root, "packages/sdk/src/index.ts"), "@datars/std": join(root, "packages/std/src/index.ts") } }).catch(() => process.exit(1));
  const mod = await import(pathToFileURL(out).href);
  process.stdout.write(JSON.stringify(await embedLocalRecipes(mod.default, dirname(src))));
} catch (e) {
  // An error the document threw while building its graph: the message, not the bundler's stack.
  process.stderr.write(`${src}: ${e instanceof Error ? e.message : String(e)}\n`);
  process.exitCode = 1;
} finally {
  rmSync(dir, { recursive: true, force: true });
}
