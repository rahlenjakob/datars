# Security policy

## Supported versions

datars hasn't reached 1.0. Security fixes land on the latest `main` and go into the next release.
Older snapshots and releases aren't patched.

| Version | Supported |
|---|---|
| latest `main` | yes |
| anything older | no |

## Reporting a vulnerability

**Please don't report security problems in public issues, discussions or pull requests.**

Report them privately with GitHub's private vulnerability reporting: open
<https://github.com/rahlenjakob/datars/security/advisories/new>, or go to the repository's **Security** tab
and choose **Report a vulnerability**. Only the maintainers can see the report.

<!-- TODO(maintainers): enable "Private vulnerability reporting" in the repository settings
     (Settings → Code security) before publishing, and add a fallback e-mail address here if wanted. -->

Please include:

- what's affected: the crate or package, the commit or version, and the platform (web/wasm, iOS,
  Android, desktop, CLI);
- how to reproduce it, ideally with a minimal document, bundle or command;
- what an attacker gains (code execution, escaping the sandbox, reading files, denial of service, …);
- whether the issue is already public or being exploited.

## What to expect

These are targets, not guarantees. The project is maintained by a small team.

- **Acknowledgement** within 3 working days.
- **Initial assessment** (confirmed or not, severity, next steps) within 10 working days.
- **Updates** at least every two weeks until the issue is resolved.
- **Fix and disclosure:** we'll agree on a disclosure date with you. By default we aim to publish a
  fix and a GitHub security advisory within 90 days of the report, sooner for serious issues. We'll
  credit you in the advisory unless you ask us not to.

## Scope

In scope: code in this repository, especially the parts that handle untrusted input.

- **The recipe and kernel sandbox**, `crates/datars-sandbox`. T3 bundles and raw source documents
  run recipe code in QuickJS inside the engine, natively and compiled into the web runtime's
  WebAssembly. Anything that escapes it is in scope: reaching IO, the clock, randomness or host
  memory beyond the documented `host(…)` interface, or getting around its time and memory budgets.
- **Bundle loading and parsing**, in `crates/datars-bundle`, `crates/datars-runtime` and the
  hosts. This covers manifest signature (ed25519) and chunk hash (BLAKE3) checks that can be
  bypassed, and malformed bundles, documents (`crates/datars-ir`), data files (CSV/JSON), fonts,
  GeoJSON/TopoJSON or PMTiles/MVT tiles that cause memory unsafety, a crash of the host app, or
  unbounded memory or CPU use.
- **The C ABI and its wrappers**, `crates/datars-ffi`, `apple/DatarsKit` and `android/`. Examples
  are memory-safety problems reachable through the documented API, and JNI or Swift bridging
  issues.
- **The web runtime**, `packages/web` and `crates/datars-host-web`. Examples are script injection
  through chart content (text, links, semantics mirrored into the DOM) and bundles reaching
  endpoints the page's policy doesn't allow.
- **The CLI's servers and fetchers**: `datars dev` and `datars serve` (including the image server
  at `/render/…`). Examples are path traversal outside the served directory and request handling
  that crashes or hangs the server. The build-time fetchers are in scope too: fonts in
  `crates/datars-build` and open geodata in `crates/datars-geo-build`.

Out of scope:

- Problems that need an attacker who can already change the local files, the CLI's arguments or
  the host app's code.
- Exposing `datars dev` or `datars serve` to an untrusted network on purpose. Both bind to
  `127.0.0.1` by default and are development tools, not hardened production servers.
- Visual differences between platforms or GPUs. These are bugs, not vulnerabilities, so please
  open a regular issue.
- Vulnerabilities in third-party dependencies that datars doesn't make reachable. Please report
  those upstream. If datars exposes one, report it here.
