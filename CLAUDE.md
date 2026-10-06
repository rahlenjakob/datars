# datars (public) — working notes for humans and coding agents

Proprietary for now (LICENSE), the source published to be read. Design docs: `docs/README.md` (read `02-principles.md` first).
Crate API contracts for parallel work: `docs/dev/contracts.md`.

## Commands

```sh
cargo test --workspace                 # all Rust tests
cargo test -p datars-scene             # one crate
cargo clippy --workspace               # includes determinism lints (clippy.toml)
target/release/datars test             # visual + motion goldens over examples/ (--update to accept)
target/release/datars gpu              # wgpu vs CPU reference (ΔE), needs a GPU
node --test packages/web/test/*.test.mjs   # wasm runtime: goldens + delivery (after scripts/build-wasm.sh)
scripts/test-android.sh                # C ABI goldens on an Android emulator
target/release/datars budgets          # bundle + runtime sizes vs budgets.json
```

Working on a document or recipe (all accept `doc.ts` or `doc.json`):

```sh
datars dev doc.ts          # live page: rebuild on save, states, diagnostics, lint
datars check|render|inspect|film|semantics|lint|profile doc.ts
datars explain doc.ts --key '("SE",)'   # why an element looks the way it does (recipe, row, expression values)
datars data profile data.csv            # types, gaps, ranges, candidate keys, hints — before charting
datars new mychart --template story     # a working doc.ts to start from (chart|story|explorable|map)
datars describe std/bar                 # a recipe's params, defaults and tokens
datars diff doc.ts --states bars,pie    # what changes between two states (or two docs)
datars migrate doc.json --write         # upgrade to the current IR; report ignored fields
datars bundle inspect out/x.datars      # variants, sizes, and what each runtime plays
datars eject std/bar       # a std recipe as editable TypeScript in recipes/
datars replay session.json # a recorded host session, replayed exactly
datars publish doc.ts --alias x --to site/ && datars serve site/   # delivery, locally
```

Status and evidence: `docs/19-status.md`. Agents can use the same tools over MCP (`datars-mcp`).
`llms.txt` and `docs/reference/std.md` are generated (`datars docs`; a test fails when they're stale).

**Accepting goldens:** only with every other test green (`cargo test --workspace` first), and after
looking at the renders. Goldens are the cross-target oracle — wasm, iOS, Android and the tier tests
compare against them — so a bad golden hides a regression everywhere at once.

## Rules that are not optional

- **Determinism (P1).** Engine crates never call `f64::sin/cos/tan/atan2/exp/ln/powf/cbrt/hypot/…` —
  use `datars_math::m::*`. `sqrt`, `floor`, `ceil`, `round`, `abs` are fine. No
  `std::collections::HashMap/HashSet` in engine crates (use `BTreeMap`/`BTreeSet`). No time, no
  randomness except `datars_math::Rng` with an explicit seed. No IO in engine crates (sans-IO, P10):
  bytes in, values out.
- **No chart vocabulary in engine crates (P4).** Words like bar, pie, axis, legend, story belong in
  the standard library (`packages/std`, `datars-algo` algorithms are fine: they're algorithms).
- **Colours are inks.** Scene colours are `datars_theme::Ink` (`"#hex"`, `"$token"`,
  `"$palette[i]"`), resolved at flatten time. Never bake theme colours into scenes.
- **Every feature has a test.** Unit tests in the crate; integration tests in `tests/`.
- **Dependencies:** workspace-level `[workspace.dependencies]` in `Cargo.toml`; pure Rust,
  deterministic, wasm-friendly, permissive licences (P15). Ask before adding heavy ones.
- Match the surrounding code: small modules, doc comments that say *why*.

## Layout

`crates/*` are all workspace members (glob). `packages/*` are npm packages (pnpm workspace).
This repository must stay self-contained: nothing here may depend on code outside it.
