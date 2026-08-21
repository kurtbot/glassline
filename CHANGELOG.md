# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0] — 2026-08-21

Two big beats in one release: an interactive layout editor (`glassline-tui`) and
full widget-catalog parity with upstream ccstatusline. Everything since v0.5.1.

### Added

**Interactive layout editor (`glassline-tui`).** Bare `glassline` in a terminal
(no piped stdin) launches it; piped input still renders the status line as
before. Built on ratatui 0.30 + tui-input + ansi-to-tui, in the workspace as
its own binary.

- Ten-entry main menu: **Edit Lines · Powerline · Terminal Options · Update
  Checker · Import / Export · Install / Uninstall · Diagnostics · Save · Quit**
  (Global Defaults deferred).
- Live-preview strip on every screen — renders through the real hot path;
  colors survive via `ansi-to-tui`.
- Widget picker with category-tinted rows and a fuzzy filter.
- Widget editor with typed knobs (Color · Text · Choice · Integer · Bool).
- Color menu: **Basic16 · Ansi256 · Truecolor** (Tab cycles modes; hex input).
- First-run wizard: welcome → template pick (Minimal / Dev / Power user, with
  live preview per template) → color-level confirm (auto-detected from
  `$COLORTERM` / `$TERM` / `$WT_SESSION`) → optional `glassline install --user`.
- Powerline setup screen — enable / separator glyph / theme / auto-align /
  continue-across-lines.
- Terminal options — flex mode / compact threshold / git cache TTL / minimalist
  mode, with `←/→` inline stepping.
- Update-checker cadence UI — interval-hours + daily-at-hour rows (schema-only
  until the periodic check ships in the render binary).
- Import / Export — auto-detect ccstatusline, import from an arbitrary path,
  export scratch settings; folder-only paths get `my-glassline-export.json`
  appended; outcomes shown as blocking `InfoModal`s.
- Install menu — shells out to `glassline install --user` / `--project` /
  `uninstall --user` with a confirm modal and a live wiring probe against
  `~/.claude/settings.json`.
- Diagnostics screen — config resolution path, WIDGETS↔META parity, env vars,
  Claude Code wiring status, tail of `~/.cache/glassline/debug.log`.
- Screen-local revert — `Esc` in the widget editor drops in-screen edits;
  `Ctrl-S` keeps them; scratch settings are only committed on `Save`.

**Widget catalog parity — 90 canonical widgets + 6 upstream aliases.** All
upstream ccstatusline widgets ported and registered:

- `sandbox-status`, `voice-status`, `remote-control-status`,
  `claude-account-email`, `vim-mode`, `cache-timer`
- `git-origin-host` — new upstream-parity widget

**`workspace.repo` native fast path.** `StatusJson` now surfaces a
`WorkspaceRepo` struct with `remote_url` / `host`; three git-origin widgets
(`git-origin`, `git-origin-owner`, `git-origin-repo`) rewired to the native
resolver with a shell-out fallback, plus a new `get_git_origin_host` resolver.

**CLI flags.** `glassline-tui` accepts `--config` / `--dry-run` / `--import` /
`--export` / `--version` / `--help` for scripted workflows.

**Animation.** `pulseAbove` metadata key gates pulse effects on rendered
percent; token / cache widgets attach a hidden percent-hint sentinel span so
threshold/pulse effects compose without changing visible text.

**Rendering.**

- Internal render cache — FNV-1a keyed on stdin + settings + time bucket.
- `glassline import` subcommand — migrates ccstatusline configs with
  `--dry-run` / `--from` / `--to` / `--yes` / `--force`.

**CI.** Workflow now fires on both `main` and `develop`.

### Changed

- Settings serialization uses `skip_serializing_if = "Option::is_none"` on all
  `Option` fields — saved `settings.json` stays compact.
- `git-branch` shows short SHA on detached HEAD.
- `pulse` on named colors resolves to actual RGB before compositing.
- Preview panels size to line count (up to 6 lines) instead of a fixed 3.
- `README.md` gains a "Configure the layout" section covering the editor,
  wizard, subscreens, and non-interactive flags.

### Fixed

- Windows crossterm Press/Release event duplication — filtered to
  `KeyEventKind::Press` at the DSL event loop.
- Insert-index panic when adding a widget to an empty row.
- Export outcome no longer leaks `eprintln!` into the alt-screen — read-only
  screens now use `Action::WithSettings` + `InfoModal`.
- `Block reset: 0m` renders — `ctx.block_metrics` is now populated from
  `usage_data.session_reset_at`.
- `List::set_filter` no-op detection preserves selection across identical
  filter strings.

### Attribution

Rust port of [ccstatusline](https://github.com/sirmalloc/ccstatusline) by
Matthew Breedlove (© 2025), MIT-licensed. Port © 2026 Kurt Milan, also MIT.

## [0.5.1] — 2026-08-01

Prior release. See `git log v0.5.0..v0.5.1` for details.

## [0.5.0] — 2026-07-15

Initial tagged release.

[0.6.0]: https://github.com/kurtbot/glassline/releases/tag/v0.6.0
[0.5.1]: https://github.com/kurtbot/glassline/releases/tag/v0.5.1
[0.5.0]: https://github.com/kurtbot/glassline/releases/tag/v0.5.0
