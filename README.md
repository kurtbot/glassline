# glassline

## Overview

Rust port of [ccstatusline](https://github.com/sirmalloc/ccstatusline) — a customizable status line formatter for the Claude Code CLI. Reads Claude Code's `StatusJSON` payload on stdin and writes an ANSI status line to stdout: current model, context-window usage, git state, session/weekly usage percentages, token throughput, and more, driven by a JSON config that matches the upstream schema.

## Attribution

This project is a Rust port of [ccstatusline](https://github.com/sirmalloc/ccstatusline) by **Matthew Breedlove** (© 2025), distributed under the MIT License. Structure, widget behavior, response-parsing semantics, and the OAuth usage-endpoint contract are modeled on that upstream. A verbatim copy of the upstream copyright notice is included as [`LICENSE-UPSTREAM`](./LICENSE-UPSTREAM), as MIT requires.

The port itself — Rust module layout, all code in `crates/`, the cross-process lock design, and the CI/tooling — is original work © 2026 Kurt Milan, also under MIT (see [`LICENSE`](./LICENSE)).

If you fork or reuse this port, please preserve BOTH copyright notices and this attribution paragraph. Stripping them is a license violation.

## Installation

Requires Rust 1.96.0 (pinned in `rust-toolchain.toml`).

```bash
cargo install --path crates/glassline-render
glassline install        # wires the binary into ~/.claude/settings.json
```

Uninstall with `glassline uninstall`.

Prebuilt binaries for Linux, macOS, and Windows are published on the [Releases](https://github.com/kurtbot/glassline/releases) page once tagged.
