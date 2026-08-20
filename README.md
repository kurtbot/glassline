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

## Migrating from ccstatusline

If you already have a working `ccstatusline` config, `glassline import` picks it up and migrates it in one shot — no hand-copy, no schema archaeology.

```bash
glassline import                          # auto-detect + prompt
glassline import --dry-run                # preview only, no writes
glassline import --from /path/settings.json --to /path/glassline.json --yes
```

The importer probes six paths for a ccstatusline source (`$CCSTATUSLINE_CONFIG`, XDG, `~/.config/ccstatusline`, legacy `~/.claude/ccstatusline`, `%APPDATA%`, `%LOCALAPPDATA%`) and writes to glassline's own config path atomically under a `settings.lock`. The original ccstatusline file is never modified — deleting the freshly-written glassline `settings.json` reverts you.

Exit codes: `0` ok · `1` no source · `2` parse/migrate error · `3` target exists without `--force` · `4` write error.

## Animation

Effects like coloured thresholds and pulses are opt-in per widget via `settings.json` metadata. Nothing animates by default.

Per-widget metadata keys (`WidgetSpec.metadata`, values are strings):

| Key             | Value                              | Effect                                                |
|-----------------|------------------------------------|-------------------------------------------------------|
| `animate`       | `rainbow`, `pulse`, `sweep`        | Unconditional time-based colour cycle                 |
| `cycleSeconds`  | `"30"`                             | Cycle length (default 60)                             |
| `thresholds`    | `"50:green,80:yellow,100:red"`     | Colour picker keyed on rendered percent (ascending)   |
| `pulseAbove`    | `"80"`, `"80%"`, `"0.8"`           | Pulse only when rendered percent ≥ N — composes with `thresholds` |
| `gradientStart` | `"#rrggbb"`                        | Start colour for a static or sweep gradient           |
| `gradientEnd`   | `"#rrggbb"`                        | End colour for a static or sweep gradient             |

Thresholds and `pulseAbove` need a percent to work against. Widgets that render a percent directly (`context-percentage`, `session-usage`, `weekly-*`) read it from their own text. Token/cache widgets (`context-length`, `tokens-*`, `cache-*`) attach a hidden percent hint of context-window occupancy, so the same effects fire on them without changing the visible text.

**Two examples.** Pulse the context-length widget when it crosses 85% of the window:

```json
{
  "type": "context-length",
  "color": "blue",
  "metadata": { "pulseAbove": "85" }
}
```

Compose with a colour ramp — cyan under 60%, yellow to 80%, bright red to 90%, flashing red past 90%:

```json
{
  "type": "context-percentage",
  "metadata": {
    "thresholds": "60:cyan,80:yellow,90:brightRed,100:#ff0000|#8b0000",
    "pulseAbove": "85"
  }
}
```

The `|`-separated form under `thresholds` alternates one colour per second — a flashing warning band.

### Animation cadence limitation

Claude Code samples the status line on user/tool events and on its configured `refreshInterval` when idle. Every animation effect (`pulse`, `rainbow`, `sweep`, flashing `thresholds`, `pulseAbove`) advances one frame per sample — visibly smooth during active work, frozen when idle.

To see smoother animation while idle, lower `refreshInterval` (in `~/.claude/settings.json`, inside the `statusLine` object). The value is in **seconds**, minimum `1` (see [Claude Code docs](https://code.claude.com/docs/en/statusline)). `glassline install` prints a hint when it detects a value ≥ 5 seconds.
