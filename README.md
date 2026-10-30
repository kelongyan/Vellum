<p align="center">
  <img src="assets/app-icon/hicolor/256x256/apps/io.github.kelongyan.vellum.png" width="128" height="128" alt="Vellum icon: a golden retriever puppy on a navy tile">
</p>

<h1 align="center">Vellum</h1>

<p align="center">
  <b>A calm, fast PDF workbench written in pure Rust.</b><br>
  Read, organize, annotate, fill, protect and automate PDFs on Windows, macOS, Linux, FreeBSD and the web.
</p>

<p align="center">
  <img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-12a58a?style=flat-square">
  <img alt="Written in Rust" src="https://img.shields.io/badge/written%20in-Rust-0a7563?style=flat-square">
  <img alt="No account, no telemetry" src="https://img.shields.io/badge/no%20account-no%20telemetry-0a7563?style=flat-square">
  <br>
  <a href="https://github.com/kelongyan/Vellum"><b>github.com/kelongyan/Vellum</b></a>
</p>

---

## About this fork

Vellum is **kelongyan's fork** of [PdfCraft](https://github.com/storytold/pdfcraft), the open-source
PDF reader and editor by the ArtCraft team. The engine, renderer, automation tools and most of the
documentation are upstream's work. This fork carries the Vellum name, icon, application identity and
its own Windows packaging.

- **Source and issues:** [github.com/kelongyan/Vellum](https://github.com/kelongyan/Vellum)
- **Upstream:** [github.com/storytold/pdfcraft](https://github.com/storytold/pdfcraft)
- **Development notes:** [docs/development.md](docs/development.md)

The app, the command-line tool and the engine share one Rust workspace. The user-facing binary is
`vellum`; the internal crate and package names remain `pdfcraft-*` so the fork stays easy to merge
with upstream.

## What it does

- **Read anything, beautifully.** Kerning, ligatures, right-to-left and complex scripts, vertical
  CJK, colour emoji, shadings, blend modes, soft masks and optional content. Large pages render in
  tiles, so deep zoom stays sharp, and damaged files are repaired rather than rejected.
- **Find and navigate.** Search as you type, select text in reading order, use bookmarks, page
  thumbnails and the document's own page labels, and reopen closed tabs.
- **Organize pages.** See every page at once, then rotate, delete, insert, move, extract, combine
  and split. Undo runs deep, saves are incremental, atomic and verified, and unsaved work is
  autosaved and recovered after a crash.
- **Comment, fill, sign.** Threaded comments, text markup, shapes and ink; form fields with
  regenerated appearances; layers and attachments; passwords and permissions; redaction.
- **Stay private.** No account, no telemetry, no cloud. Documents never leave your machine.
- **Run everywhere.** Native on Windows, macOS, Linux and FreeBSD, and in the browser through
  WebAssembly, from the same codebase.

Feature details and screenshots live in the upstream documentation; this fork keeps that behaviour
and changes the product identity around it.

## Build from source

Building needs Rust 1.95 or later.

```sh
git clone https://github.com/kelongyan/Vellum
cd Vellum
cargo xtask models                                 # OCR models for Scan & OCR
cargo run --release -p pdfcraft -- some.pdf        # desktop app; the binary is vellum
```

Japanese interface text and Japanese text written into PDFs need the optional
[craft-fonts](https://github.com/storytold/craft-fonts) checkout:

```sh
git clone https://github.com/storytold/craft-fonts ../craft-fonts
CRAFT_FONTS_DIR=../craft-fonts cargo run --release -p pdfcraft -- some.pdf
```

On Windows, the packaging script builds the MSI and the portable zip:

```powershell
$env:CRAFT_FONTS_DIR = '..\craft-fonts'   # optional: embeds the Japanese fonts
.\packaging\windows\package.ps1 -Arch x64
```

The installers and executables it produces are named `vellum.exe`, `vellum-cli.exe`,
`vellum-<version>-windows-x64.msi` and `vellum-<version>-windows-x64-portable.zip`.

## Command line and agents

`vellum-cli` exposes the engine without opening a window:

```sh
vellum-cli info  form.pdf                                  # structure as JSON
vellum-cli text  paper.pdf --page 3                        # reading-order text
vellum-cli edit  in.pdf --rotate 1,2:90 --delete 5 --title "Q3" --out out.pdf
vellum-cli combine report.pdf appendix.pdf --out combined.pdf
vellum-cli extract report.pdf --pages 1,3,5-8 --out highlights.pdf
vellum-cli split   report.pdf --every 10 --out-dir parts/
```

Every engine feature is also reachable through one table of JSON-Schema-described tools:

```sh
vellum-cli tools                                        # every tool and its JSON Schema
vellum-cli run text_find doc=1 query=invoice            # key=value; values parse as JSON
vellum-cli run --script steps.json --root ./work        # several steps in one session
```

An opt-in MCP server is available for AI agents. It never starts on its own, opens no network
port, and runs only while an agent launches `vellum-cli mcp`:

```json
{ "mcpServers": { "vellum": { "command": "vellum-cli", "args": ["mcp", "--root", "/path/to/your/pdfs"] } } }
```

The desktop app can also be driven through its off-by-default control channel:

```sh
vellum --control ~/.vellum-control.json document.pdf
vellum-cli ui --control ~/.vellum-control.json inspect query=rotate
vellum-cli ui --control ~/.vellum-control.json screenshot --out window.png
```

## Repository layout

| Path | What it holds |
|---|---|
| `crates/` | The engine: filters, crypt, object layer, organize, fonts, annotations, forms, render, engine façade, automation and UI |
| `apps/pdfcraft/` | The desktop and web app; builds the `vellum` binary |
| `apps/pdfcraft-cli/` | `vellum-cli`, including the opt-in MCP server |
| `packaging/` | Windows, macOS, Linux, FreeBSD and web packaging |
| `docs/` | Architecture, development, releasing, parity and localization notes |
| `xtask/` | Repository tasks: models, demo PDF, screenshots, asset audit, quality gates |

## Status

Vellum is **alpha**, the same stage as upstream PdfCraft. Viewing, search, organizing, combining,
splitting, commenting, filling forms, passwords and basic signatures work today. Reliable editing of
existing text (especially CJK and right-to-left), office-document conversion, OCR beyond Latin
script, Preflight/PDF-X/UA and full XFA forms are still thin or missing. The honest, detailed
assessment is upstream's: [ROADMAP.md](https://github.com/storytold/pdfcraft/blob/main/ROADMAP.md),
[docs/target-app-parity.md](https://github.com/storytold/pdfcraft/blob/main/docs/target-app-parity.md)
and [docs/gaps.md](https://github.com/storytold/pdfcraft/blob/main/docs/gaps.md).

Each change passes formatting and Clippy with warnings as errors, 200+ unit, property and UI tests,
crate-layering checks, a WebAssembly build check and an asset-licence audit. Rendering and
round-trip output are checked against independent implementations (hayro, qpdf and, on demand,
poppler). Vellum is a clean-room implementation: behaviour comes from the ISO 32000 specification
and black-box observation, never from anyone else's code.

## Downloads

This fork does not publish its own GitHub releases yet. Build from source with the commands above,
or run the Windows packaging script to produce the MSI and portable zip locally.

## License

Vellum is dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
Copyright (c) 2026 ArtCraft Team and the PdfCraft contributors. Required notices are in
[NOTICE](NOTICE).

Bundled fonts, icons, images and other assets keep their own open licences; each one is listed with
its author, source and licence in [ATTRIBUTION.md](ATTRIBUTION.md). Release builds also embed the
Japanese fonts of [craft-fonts](https://github.com/storytold/craft-fonts/blob/main/ATTRIBUTION.md)
(SIL Open Font License 1.1).

## Acknowledgements / 致谢

Vellum exists because of the PdfCraft authors and the ArtCraft team. Thank you for building an open,
careful PDF engine and for licensing it under MIT OR Apache-2.0, so that forks like this one can
exist. Thanks also to every PdfCraft contributor, and to the maintainers of the open-source projects
it builds on: hayro, egui, Lucide, and the OFL font authors.

感谢 PdfCraft 的原作者与 ArtCraft 团队：Vellum 的引擎、渲染器、自动化工具和大部分文档都来自
他们的工作。也感谢所有上游贡献者，以及 hayro、egui、Lucide 与 OFL 字体作者等开源项目。
