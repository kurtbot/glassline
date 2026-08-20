# glassline

## Overview

Rust port of [ccstatusline](https://github.com/sirmalloc/ccstatusline) — a customizable status line formatter for the Claude Code CLI. Reads Claude Code's `StatusJSON` payload on stdin and writes an ANSI status line to stdout: current model, context-window usage, git and jujutsu state, session/weekly usage percentages, token throughput, PR/CI status, and more — driven by a JSON config that matches the upstream schema.

**Widget catalog:** 83 of 87 upstream widget IDs (~95%) resolve in the built-in registry. The four deferred (`vim-mode`, `voice-status`, `sandbox-status`, `remote-control-status`, `claude-account-email`, `cache-timer`) need scanner extensions, IPC protocols, or filesystem watchers that don't exist yet.

**Hardening:** cross-process lock file, macOS Keychain fallback for OAuth token, `HTTPS_PROXY` / `NO_PROXY` resolution, per-outcome cache TTL, `Retry-After` honoring on 429.

## Attribution

This project is a Rust port of [ccstatusline](https://github.com/sirmalloc/ccstatusline) by **Matthew Breedlove** (© 2025), distributed under the MIT License. Structure, widget behavior, response-parsing semantics, and the OAuth usage-endpoint contract are modeled on that upstream. A verbatim copy of the upstream copyright notice is included as [`LICENSE-UPSTREAM`](./LICENSE-UPSTREAM), as MIT requires.

The port itself — Rust module layout, all code in `crates/`, the cross-process lock design, and the CI/tooling — is original work © 2026 Kurt Milan, also under MIT (see [`LICENSE`](./LICENSE)).

If you fork or reuse this port, please preserve BOTH copyright notices and this attribution paragraph. Stripping them is a license violation.

## Installation

Pick whichever channel fits. Every option installs the same binary — after any of them, run `glassline install` to wire it into `~/.claude/settings.json`.

### Universal one-liner (macOS / Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/kurtbot/glassline/main/packaging/install.sh | sh
```

Downloads the archive matching your OS+arch from the latest release, verifies its SHA256, extracts to `~/.local/bin/glassline`.

Pin a version: `sh -s -- --version v0.5.1`. Override dir: `sh -s -- --dir /usr/local/bin`.

### Windows (PowerShell)

```powershell
iwr https://raw.githubusercontent.com/kurtbot/glassline/main/packaging/install.ps1 -UseBasicParsing | iex
```

Installs to `%LOCALAPPDATA%\glassline\glassline.exe`. Same SHA256 verification.

### Homebrew (macOS + Linuxbrew)

```bash
brew tap kurtbot/glassline https://github.com/kurtbot/glassline.git
brew install glassline
```

### cargo-binstall (Rust ecosystem)

Fetches the prebuilt binary instead of source-compiling:

```bash
cargo binstall --git https://github.com/kurtbot/glassline glassline-render
```

(The `--git` flag is required because the crates are workspace-only — not published to crates.io.)

### From source

Requires Rust 1.96.0 (pinned in `rust-toolchain.toml`).

```bash
cargo install --path crates/glassline-render
```

### After install

```bash
glassline install     # writes ~/.claude/settings.json statusLine hook
glassline --version   # confirm
glassline uninstall   # revert
```

Prebuilt raw archives + `SHA256SUMS.txt` are on the [Releases](https://github.com/kurtbot/glassline/releases) page if you'd rather download manually.
