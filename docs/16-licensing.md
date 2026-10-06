# 16 — Licensing: what's published, what's hosted

## The source is published, not licensed

datars is proprietary for now: © 2026 Jakob Råhlén, all rights reserved ([LICENSE](../LICENSE)).
The engine, its renderers and hosts (web, iOS, macOS, Android, desktop, headless), the bundle format
and loader, the publish compiler, the standard library, the SDKs, the CLI and dev tools, the MCP
server and the map-data pipeline are all in this repository so they can be read. Using, copying or
distributing them needs written permission.

**Self-hosting is built in.** A published chart is static files: any bucket, CDN or web server that
answers range requests can host it, and the runtime plays it. No account, no API key, no chart
server.

## What may be hosted on top

Services such as a visual editor, managed publishing (a bundle CDN, aliases, rollbacks, key
management), data connectors and managed tile hosting can be built on top of datars. They are held
to one rule:

> A hosted service uses only the public engine API, the bundle format and the dev-tools protocol —
> no private forks, no hidden hooks. If a service needs something, the public API gets it first.

This is dogfooding (P5) applied beyond the standard library: nothing built on datars can do what the
tools in this repository can't, and a chart made with a hosted tool is an ordinary bundle the
runtime plays.

## Licensing notes

- **Code:** proprietary ([LICENSE](../LICENSE)). Pull requests aren't accepted while it is; issues
  and discussions are welcome ([CONTRIBUTING](../CONTRIBUTING.md)).
- **Fonts** bundled by default (Inter) are under the SIL Open Font License; a theme may name other
  fonts, and `datars fonts` / `datars lint` report their licences and embedding restrictions.
- **Map data:** Natural Earth is public domain; OpenStreetMap data is © OpenStreetMap contributors
  under the ODbL — rendered maps need attribution (the std `attribution` recipe draws it) and
  distributed tile archives carry share-alike obligations.
- Third-party material in this repository is listed in [THIRD_PARTY_NOTICES](../THIRD_PARTY_NOTICES.md).

## Stability

- Crates and packages follow semver; before 1.0, minor versions may break APIs.
- The document format (the IR) is versioned separately; every breaking change ships with a migration
  (`datars migrate`), so old documents keep working.
- Published bundles declare the runtime features they need; runtimes play what they support and fall
  back to the poster and accessible text otherwise ([12](12-delivery.md)).
