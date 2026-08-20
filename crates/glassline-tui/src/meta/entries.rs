//! `WidgetMeta` statics for every canonical widget in
//! `glassline_widgets::registry::WIDGETS`. Aliases (`worktree-branch`
//! etc.) inherit their canonical widget's metadata and do not appear
//! here; the drift test in `super::drift` enforces the split.
//!
//! Layout: shared knob-set consts first, then one `static META_*` per
//! widget grouped by category, then the phf `METAS` map at the bottom.

use phf::phf_map;

use super::{MetaKnob, MetaShape, Styling, ValueKnob, WidgetCategory, WidgetKnob, WidgetMeta};

// ---------------------------------------------------------------------------
// Shared knob-set constants — attach these to entries via `&SHARED_*`.
// ---------------------------------------------------------------------------

/// `pulseAbove` — percentage threshold that triggers pulse animation on
/// context / cache / token / usage widgets. `spec.metadata["pulseAbove"]`
/// as a percent integer string (e.g. `"80"`).
const KNOB_PULSE_ABOVE: WidgetKnob = WidgetKnob::Meta(MetaKnob {
    key: "pulseAbove",
    label: "Pulse above %",
    shape: MetaShape::Choice {
        options: &["", "50", "60", "70", "75", "80", "85", "90", "95"],
    },
});

/// `thresholds` — comma-separated `pct:color` bands for context/usage widgets.
const KNOB_THRESHOLDS: WidgetKnob = WidgetKnob::Meta(MetaKnob {
    key: "thresholds",
    label: "Color thresholds",
    shape: MetaShape::Text {
        hint: "30:green,60:yellow,80:red",
        max_len: 200,
    },
});

/// `hideNoGit` — hide widget when the cwd is outside a git repo.
const KNOB_HIDE_NO_GIT: WidgetKnob = WidgetKnob::Meta(MetaKnob {
    key: "hideNoGit",
    label: "Hide outside git",
    shape: MetaShape::Bool {
        default_when_absent: false,
    },
});

/// `useNerdFont` — enable nerd-font glyph rendering on format-variant widgets.
const KNOB_USE_NERD_FONT: WidgetKnob = WidgetKnob::Meta(MetaKnob {
    key: "useNerdFont",
    label: "Use nerd font glyphs",
    shape: MetaShape::Bool {
        default_when_absent: false,
    },
});

/// `hideWhenEmpty` — used by cache-timer + a few others; hides render
/// when the underlying value is absent.
const KNOB_HIDE_WHEN_EMPTY: WidgetKnob = WidgetKnob::Meta(MetaKnob {
    key: "hideWhenEmpty",
    label: "Hide when empty",
    shape: MetaShape::Bool {
        default_when_absent: false,
    },
});

// Shared knob sets — the picker/editor iterates these slices per widget.
const KNOBS_CONTEXT_ANIMATED: &[WidgetKnob] = &[KNOB_THRESHOLDS, KNOB_PULSE_ABOVE];
const KNOBS_HIDE_NO_GIT: &[WidgetKnob] = &[KNOB_HIDE_NO_GIT];

// ---------------------------------------------------------------------------
// Model / meta
// ---------------------------------------------------------------------------

static META_MODEL: WidgetMeta = WidgetMeta {
    id: "model",
    label: "Model name",
    category: WidgetCategory::Model,
    description: "Active Claude model's display name.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_VERSION: WidgetMeta = WidgetMeta {
    id: "version",
    label: "Claude Code version",
    category: WidgetCategory::Model,
    description: "The Claude Code version reported by the host session.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_OUTPUT_STYLE: WidgetMeta = WidgetMeta {
    id: "output-style",
    label: "Output style",
    category: WidgetCategory::Model,
    description: "Current output style (`default`, `explanatory`, ...).",
    styling: Styling::Standard,
    knobs: &[],
};

static META_THINKING_EFFORT: WidgetMeta = WidgetMeta {
    id: "thinking-effort",
    label: "Thinking effort",
    category: WidgetCategory::Model,
    description: "Reasoning effort level (`low`/`medium`/`high`/`xhigh`/`max`).",
    styling: Styling::Standard,
    knobs: &[],
};

// ---------------------------------------------------------------------------
// Context window
// ---------------------------------------------------------------------------

static META_CONTEXT_BAR: WidgetMeta = WidgetMeta {
    id: "context-bar",
    label: "Context bar",
    category: WidgetCategory::Context,
    description: "Segmented context-usage progress bar.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

static META_CONTEXT_PERCENTAGE: WidgetMeta = WidgetMeta {
    id: "context-percentage",
    label: "Context percentage",
    category: WidgetCategory::Context,
    description: "Percentage of the model's context window in use.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

static META_CONTEXT_PERCENTAGE_USABLE: WidgetMeta = WidgetMeta {
    id: "context-percentage-usable",
    label: "Context percentage (usable)",
    category: WidgetCategory::Context,
    description: "Percentage of the context window still usable after reserving max output.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

static META_CONTEXT_LENGTH: WidgetMeta = WidgetMeta {
    id: "context-length",
    label: "Context length",
    category: WidgetCategory::Context,
    description: "Absolute token count currently in the context window.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

static META_CONTEXT_WINDOW: WidgetMeta = WidgetMeta {
    id: "context-window",
    label: "Context window size",
    category: WidgetCategory::Context,
    description: "The model's max context window (e.g. 200k).",
    styling: Styling::Standard,
    knobs: &[],
};

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

static META_TOKENS_INPUT: WidgetMeta = WidgetMeta {
    id: "tokens-input",
    label: "Tokens (input)",
    category: WidgetCategory::Tokens,
    description: "Input tokens for the most recent turn.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

static META_TOKENS_OUTPUT: WidgetMeta = WidgetMeta {
    id: "tokens-output",
    label: "Tokens (output)",
    category: WidgetCategory::Tokens,
    description: "Output tokens for the most recent turn.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

static META_TOKENS_CACHED: WidgetMeta = WidgetMeta {
    id: "tokens-cached",
    label: "Tokens (cached)",
    category: WidgetCategory::Tokens,
    description: "Prompt-cache read + creation tokens.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

static META_TOKENS_TOTAL: WidgetMeta = WidgetMeta {
    id: "tokens-total",
    label: "Tokens (total)",
    category: WidgetCategory::Tokens,
    description: "Input + output + cached tokens.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

static META_CACHE_READ: WidgetMeta = WidgetMeta {
    id: "cache-read",
    label: "Cache reads",
    category: WidgetCategory::Context,
    description: "Prompt-cache read tokens.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

static META_CACHE_WRITE: WidgetMeta = WidgetMeta {
    id: "cache-write",
    label: "Cache writes",
    category: WidgetCategory::Context,
    description: "Prompt-cache creation tokens.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

static META_CACHE_HIT_RATE: WidgetMeta = WidgetMeta {
    id: "cache-hit-rate",
    label: "Cache hit rate",
    category: WidgetCategory::Context,
    description: "Cache read fraction of total context.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

static META_CACHE_TIMER: WidgetMeta = WidgetMeta {
    id: "cache-timer",
    label: "Prompt-cache TTL",
    category: WidgetCategory::Context,
    description: "Countdown to prompt-cache eviction with HOT/FRESH/DRAINING/URGENT/COLD bands.",
    styling: Styling::Standard,
    knobs: &[
        WidgetKnob::Meta(MetaKnob {
            key: "ttlSeconds",
            label: "TTL seconds",
            shape: MetaShape::Integer {
                min: 60,
                max: 86_400,
                default: 300,
            },
        }),
        KNOB_HIDE_WHEN_EMPTY,
        WidgetKnob::Meta(MetaKnob {
            key: "symbolHot",
            label: "Symbol · HOT",
            shape: MetaShape::Text {
                hint: "e.g. 🔥",
                max_len: 8,
            },
        }),
        WidgetKnob::Meta(MetaKnob {
            key: "symbolFresh",
            label: "Symbol · FRESH",
            shape: MetaShape::Text {
                hint: "e.g. ✨",
                max_len: 8,
            },
        }),
        WidgetKnob::Meta(MetaKnob {
            key: "symbolDraining",
            label: "Symbol · DRAINING",
            shape: MetaShape::Text {
                hint: "e.g. ⌛",
                max_len: 8,
            },
        }),
        WidgetKnob::Meta(MetaKnob {
            key: "symbolUrgent",
            label: "Symbol · URGENT",
            shape: MetaShape::Text {
                hint: "e.g. ⚠️",
                max_len: 8,
            },
        }),
        WidgetKnob::Meta(MetaKnob {
            key: "symbolCold",
            label: "Symbol · COLD",
            shape: MetaShape::Text {
                hint: "e.g. ❄️",
                max_len: 8,
            },
        }),
    ],
};

static META_COMPACTION_COUNTER: WidgetMeta = WidgetMeta {
    id: "compaction-counter",
    label: "Compactions",
    category: WidgetCategory::Context,
    description: "Number of compactions this session (auto + manual).",
    styling: Styling::Standard,
    knobs: &[],
};

// ---------------------------------------------------------------------------
// Timing (block / speed / session-clock)
// ---------------------------------------------------------------------------

static META_BLOCK_TIMER: WidgetMeta = WidgetMeta {
    id: "block-timer",
    label: "5h block · elapsed",
    category: WidgetCategory::Timing,
    description: "Time since the current 5-hour usage block started.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_BLOCK_RESET_TIMER: WidgetMeta = WidgetMeta {
    id: "block-reset-timer",
    label: "5h block · reset in",
    category: WidgetCategory::Timing,
    description: "Time remaining until the current 5-hour block resets.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_INPUT_SPEED: WidgetMeta = WidgetMeta {
    id: "input-speed",
    label: "Input speed",
    category: WidgetCategory::Timing,
    description: "Input tokens per second (rolling window).",
    styling: Styling::Standard,
    knobs: &[],
};

static META_OUTPUT_SPEED: WidgetMeta = WidgetMeta {
    id: "output-speed",
    label: "Output speed",
    category: WidgetCategory::Timing,
    description: "Output tokens per second (rolling window).",
    styling: Styling::Standard,
    knobs: &[],
};

static META_TOTAL_SPEED: WidgetMeta = WidgetMeta {
    id: "total-speed",
    label: "Total speed",
    category: WidgetCategory::Timing,
    description: "Combined input + output tokens per second.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_SESSION_CLOCK: WidgetMeta = WidgetMeta {
    id: "session-clock",
    label: "Session duration",
    category: WidgetCategory::Timing,
    description: "Wall-clock time since the session opened.",
    styling: Styling::Standard,
    knobs: &[],
};

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

static META_SESSION_NAME: WidgetMeta = WidgetMeta {
    id: "session-name",
    label: "Session name",
    category: WidgetCategory::Session,
    description: "Human-friendly label (set via `--name` or `/rename`).",
    styling: Styling::Standard,
    knobs: &[],
};

static META_SESSION_COST: WidgetMeta = WidgetMeta {
    id: "session-cost",
    label: "Session cost",
    category: WidgetCategory::Session,
    description: "Running dollar cost for the current session.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_SESSION_USAGE: WidgetMeta = WidgetMeta {
    id: "session-usage",
    label: "5h window · usage %",
    category: WidgetCategory::Usage,
    description: "Percent of the 5-hour usage window consumed.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

static META_CLAUDE_SESSION_ID: WidgetMeta = WidgetMeta {
    id: "claude-session-id",
    label: "Session ID",
    category: WidgetCategory::Session,
    description: "The `session_id` UUID from the current status payload.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_CLAUDE_ACCOUNT_EMAIL: WidgetMeta = WidgetMeta {
    id: "claude-account-email",
    label: "Account email",
    category: WidgetCategory::Session,
    description: "The Claude.ai account email; empty when signed out.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_VIM_MODE: WidgetMeta = WidgetMeta {
    id: "vim-mode",
    label: "Vim mode",
    category: WidgetCategory::Session,
    description: "Current Vim mode when Claude Code's vim keybinds are on.",
    styling: Styling::Standard,
    knobs: &[],
};

// ---------------------------------------------------------------------------
// System / environment
// ---------------------------------------------------------------------------

static META_CWD: WidgetMeta = WidgetMeta {
    id: "current-working-dir",
    label: "Working directory",
    category: WidgetCategory::System,
    description: "The current working directory (usually the project root).",
    styling: Styling::Standard,
    knobs: &[],
};

static META_TERMINAL_WIDTH: WidgetMeta = WidgetMeta {
    id: "terminal-width",
    label: "Terminal width",
    category: WidgetCategory::System,
    description: "Detected terminal width in columns.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_FREE_MEMORY: WidgetMeta = WidgetMeta {
    id: "free-memory",
    label: "Free memory",
    category: WidgetCategory::System,
    description: "System free memory. Requires the `sysinfo-widgets` feature.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_SKILLS: WidgetMeta = WidgetMeta {
    id: "skills",
    label: "Skills",
    category: WidgetCategory::Session,
    description: "Names of skills invoked in the current session.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_LINK: WidgetMeta = WidgetMeta {
    id: "link",
    label: "Clickable link",
    category: WidgetCategory::Custom,
    description: "Emits an OSC 8 hyperlink around a static label.",
    styling: Styling::Standard,
    knobs: &[
        WidgetKnob::Value(ValueKnob {
            label: "Label",
            hint: "text shown to the user",
            max_len: 200,
        }),
        WidgetKnob::Meta(MetaKnob {
            key: "url",
            label: "URL",
            shape: MetaShape::Text {
                hint: "https://…",
                max_len: 500,
            },
        }),
    ],
};

// ---------------------------------------------------------------------------
// Git
// ---------------------------------------------------------------------------

static META_GIT_BRANCH: WidgetMeta = WidgetMeta {
    id: "git-branch",
    label: "Git branch",
    category: WidgetCategory::Git,
    description: "Current branch (falls back to short SHA on detached HEAD).",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_SHA: WidgetMeta = WidgetMeta {
    id: "git-sha",
    label: "Git short SHA",
    category: WidgetCategory::Git,
    description: "Abbreviated HEAD SHA.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_STATUS: WidgetMeta = WidgetMeta {
    id: "git-status",
    label: "Git status glyphs",
    category: WidgetCategory::Git,
    description: "Dirty-state summary as glyphs (`M`/`A`/`?`/`U`).",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_CLEAN_STATUS: WidgetMeta = WidgetMeta {
    id: "git-clean-status",
    label: "Git clean/dirty flag",
    category: WidgetCategory::Git,
    description: "Marker for whether the working tree is clean.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_CHANGES: WidgetMeta = WidgetMeta {
    id: "git-changes",
    label: "Git diff counts",
    category: WidgetCategory::Git,
    description: "Combined insertion + deletion counts (`+42,-10`).",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_INSERTIONS: WidgetMeta = WidgetMeta {
    id: "git-insertions",
    label: "Git insertions",
    category: WidgetCategory::Git,
    description: "Lines added across staged + unstaged diff.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_DELETIONS: WidgetMeta = WidgetMeta {
    id: "git-deletions",
    label: "Git deletions",
    category: WidgetCategory::Git,
    description: "Lines removed across staged + unstaged diff.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_STAGED: WidgetMeta = WidgetMeta {
    id: "git-staged",
    label: "Git staged flag",
    category: WidgetCategory::Git,
    description: "Glyph shown when the index has staged changes.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_STAGED_FILES: WidgetMeta = WidgetMeta {
    id: "git-staged-files",
    label: "Git staged files",
    category: WidgetCategory::Git,
    description: "Count of staged files.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_UNSTAGED: WidgetMeta = WidgetMeta {
    id: "git-unstaged",
    label: "Git unstaged flag",
    category: WidgetCategory::Git,
    description: "Glyph shown when the working tree has unstaged changes.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_UNSTAGED_FILES: WidgetMeta = WidgetMeta {
    id: "git-unstaged-files",
    label: "Git unstaged files",
    category: WidgetCategory::Git,
    description: "Count of files with unstaged modifications.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_UNTRACKED: WidgetMeta = WidgetMeta {
    id: "git-untracked",
    label: "Git untracked flag",
    category: WidgetCategory::Git,
    description: "Glyph shown when untracked files exist.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_UNTRACKED_FILES: WidgetMeta = WidgetMeta {
    id: "git-untracked-files",
    label: "Git untracked files",
    category: WidgetCategory::Git,
    description: "Count of untracked files.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_CONFLICTS: WidgetMeta = WidgetMeta {
    id: "git-conflicts",
    label: "Git merge conflicts",
    category: WidgetCategory::Git,
    description: "Marker for unresolved merge conflicts.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_ROOT_DIR: WidgetMeta = WidgetMeta {
    id: "git-root-dir",
    label: "Git repo root",
    category: WidgetCategory::Git,
    description: "Absolute path of the top-level repo directory.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_AHEAD_BEHIND: WidgetMeta = WidgetMeta {
    id: "git-ahead-behind",
    label: "Git ahead/behind",
    category: WidgetCategory::Git,
    description: "Commits ahead and behind of upstream.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_ORIGIN_HOST: WidgetMeta = WidgetMeta {
    id: "git-origin-host",
    label: "Git origin host",
    category: WidgetCategory::Git,
    description: "Host portion of the origin remote (`github.com`, `gitlab.com`, …).",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_ORIGIN_OWNER: WidgetMeta = WidgetMeta {
    id: "git-origin-owner",
    label: "Git origin owner",
    category: WidgetCategory::Git,
    description: "Owner portion of the origin remote URL.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_ORIGIN_REPO: WidgetMeta = WidgetMeta {
    id: "git-origin-repo",
    label: "Git origin repo",
    category: WidgetCategory::Git,
    description: "Repo name portion of the origin remote URL.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_ORIGIN_OWNER_REPO: WidgetMeta = WidgetMeta {
    id: "git-origin-owner-repo",
    label: "Git origin owner/repo",
    category: WidgetCategory::Git,
    description: "`owner/repo` pair for the origin remote.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_UPSTREAM_OWNER: WidgetMeta = WidgetMeta {
    id: "git-upstream-owner",
    label: "Git upstream owner",
    category: WidgetCategory::Git,
    description: "Owner of the `upstream` remote (fork parent).",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_UPSTREAM_REPO: WidgetMeta = WidgetMeta {
    id: "git-upstream-repo",
    label: "Git upstream repo",
    category: WidgetCategory::Git,
    description: "Repo name of the `upstream` remote.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_UPSTREAM_OWNER_REPO: WidgetMeta = WidgetMeta {
    id: "git-upstream-owner-repo",
    label: "Git upstream owner/repo",
    category: WidgetCategory::Git,
    description: "`owner/repo` pair for the `upstream` remote.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_IS_FORK: WidgetMeta = WidgetMeta {
    id: "git-is-fork",
    label: "Git is-fork flag",
    category: WidgetCategory::Git,
    description: "Marker rendered when `upstream` exists and differs from `origin`.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_CI_STATUS: WidgetMeta = WidgetMeta {
    id: "git-ci-status",
    label: "Git CI status",
    category: WidgetCategory::Git,
    description: "Latest CI-run conclusion (via `gh run list`).",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_PR: WidgetMeta = WidgetMeta {
    id: "git-pr",
    label: "Git open PR",
    category: WidgetCategory::Git,
    description: "Open PR/MR for the current branch (via `gh` / GitLab).",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_WORKTREE: WidgetMeta = WidgetMeta {
    id: "git-worktree",
    label: "Git worktree",
    category: WidgetCategory::Git,
    description: "Worktree label (name / mode / branch composite).",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_WORKTREE_BRANCH: WidgetMeta = WidgetMeta {
    id: "git-worktree-branch",
    label: "Git worktree branch",
    category: WidgetCategory::Git,
    description: "Branch of the current Claude Code worktree session.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_WORKTREE_MODE: WidgetMeta = WidgetMeta {
    id: "git-worktree-mode",
    label: "Git worktree mode",
    category: WidgetCategory::Git,
    description: "Worktree mode indicator (`--worktree` sessions).",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_WORKTREE_NAME: WidgetMeta = WidgetMeta {
    id: "git-worktree-name",
    label: "Git worktree name",
    category: WidgetCategory::Git,
    description: "Name of the current worktree.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

static META_GIT_WORKTREE_ORIGINAL_BRANCH: WidgetMeta = WidgetMeta {
    id: "git-worktree-original-branch",
    label: "Git worktree base branch",
    category: WidgetCategory::Git,
    description: "Branch the worktree was created from.",
    styling: Styling::Standard,
    knobs: KNOBS_HIDE_NO_GIT,
};

// ---------------------------------------------------------------------------
// Jujutsu
// ---------------------------------------------------------------------------

static META_JJ_REVISION: WidgetMeta = WidgetMeta {
    id: "jj-revision",
    label: "jj revision",
    category: WidgetCategory::Jj,
    description: "Current jj change-id short prefix.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_JJ_BOOKMARKS: WidgetMeta = WidgetMeta {
    id: "jj-bookmarks",
    label: "jj bookmarks",
    category: WidgetCategory::Jj,
    description: "Bookmarks pointing at the current change.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_JJ_CHANGES: WidgetMeta = WidgetMeta {
    id: "jj-changes",
    label: "jj change count",
    category: WidgetCategory::Jj,
    description: "Number of files changed in the current jj rev.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_JJ_DESCRIPTION: WidgetMeta = WidgetMeta {
    id: "jj-description",
    label: "jj description",
    category: WidgetCategory::Jj,
    description: "First line of the change's description.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_JJ_INSERTIONS: WidgetMeta = WidgetMeta {
    id: "jj-insertions",
    label: "jj insertions",
    category: WidgetCategory::Jj,
    description: "Lines added in the current change.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_JJ_DELETIONS: WidgetMeta = WidgetMeta {
    id: "jj-deletions",
    label: "jj deletions",
    category: WidgetCategory::Jj,
    description: "Lines removed in the current change.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_JJ_ROOT_DIR: WidgetMeta = WidgetMeta {
    id: "jj-root-dir",
    label: "jj repo root",
    category: WidgetCategory::Jj,
    description: "Absolute path of the jj repo root.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_JJ_WORKSPACE: WidgetMeta = WidgetMeta {
    id: "jj-workspace",
    label: "jj workspace",
    category: WidgetCategory::Jj,
    description: "Current jj workspace name.",
    styling: Styling::Standard,
    knobs: &[],
};

// ---------------------------------------------------------------------------
// Usage
// ---------------------------------------------------------------------------

static META_WEEKLY_USAGE: WidgetMeta = WidgetMeta {
    id: "weekly-usage",
    label: "7d window · usage %",
    category: WidgetCategory::Usage,
    description: "Percentage of the 7-day usage window consumed.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

static META_WEEKLY_SONNET_USAGE: WidgetMeta = WidgetMeta {
    id: "weekly-sonnet-usage",
    label: "7d Sonnet · usage %",
    category: WidgetCategory::Usage,
    description: "7-day Sonnet-only usage percentage.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

static META_WEEKLY_OPUS_USAGE: WidgetMeta = WidgetMeta {
    id: "weekly-opus-usage",
    label: "7d Opus · usage %",
    category: WidgetCategory::Usage,
    description: "7-day Opus-only usage percentage.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

static META_WEEKLY_RESET_TIMER: WidgetMeta = WidgetMeta {
    id: "weekly-reset-timer",
    label: "7d window · reset in",
    category: WidgetCategory::Usage,
    description: "Time remaining until the 7-day usage window resets.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_FABLE_WEEKLY_USAGE: WidgetMeta = WidgetMeta {
    id: "fable-weekly-usage",
    label: "Fable · weekly usage",
    category: WidgetCategory::Usage,
    description: "Fable-model weekly usage percentage.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

static META_EXTRA_USAGE_USED: WidgetMeta = WidgetMeta {
    id: "extra-usage-used",
    label: "Extra usage · spent",
    category: WidgetCategory::Usage,
    description: "Additional pay-as-you-go usage consumed.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_EXTRA_USAGE_REMAINING: WidgetMeta = WidgetMeta {
    id: "extra-usage-remaining",
    label: "Extra usage · remaining",
    category: WidgetCategory::Usage,
    description: "Additional pay-as-you-go budget still available.",
    styling: Styling::Standard,
    knobs: &[],
};

static META_EXTRA_USAGE_UTILIZATION: WidgetMeta = WidgetMeta {
    id: "extra-usage-utilization",
    label: "Extra usage · %",
    category: WidgetCategory::Usage,
    description: "Percentage of the extra-usage budget consumed.",
    styling: Styling::Standard,
    knobs: KNOBS_CONTEXT_ANIMATED,
};

// ---------------------------------------------------------------------------
// Custom / user-authored
// ---------------------------------------------------------------------------

static META_CUSTOM_TEXT: WidgetMeta = WidgetMeta {
    id: "custom-text",
    label: "Custom text",
    category: WidgetCategory::Custom,
    description: "Static text emitted verbatim.",
    styling: Styling::Standard,
    knobs: &[WidgetKnob::Value(ValueKnob {
        label: "Text",
        hint: "shown as-is",
        max_len: 200,
    })],
};

static META_CUSTOM_SYMBOL: WidgetMeta = WidgetMeta {
    id: "custom-symbol",
    label: "Custom symbol",
    category: WidgetCategory::Custom,
    description: "Single-glyph symbol (nerd-font friendly).",
    styling: Styling::Standard,
    knobs: &[WidgetKnob::Value(ValueKnob {
        label: "Symbol",
        hint: "e.g. ⚡",
        max_len: 8,
    })],
};

static META_CUSTOM_COMMAND: WidgetMeta = WidgetMeta {
    id: "custom-command",
    label: "Custom command",
    category: WidgetCategory::Custom,
    description: "Runs a shell command and renders its stdout. In-crate stopgap for external widgets.",
    styling: Styling::Standard,
    knobs: &[
        WidgetKnob::Meta(MetaKnob {
            key: "command",
            label: "Command",
            shape: MetaShape::Text {
                hint: "e.g. date +%H:%M",
                max_len: 500,
            },
        }),
        WidgetKnob::Meta(MetaKnob {
            key: "timeoutMs",
            label: "Timeout (ms)",
            shape: MetaShape::Integer {
                min: 50,
                max: 5_000,
                default: 500,
            },
        }),
    ],
};

// ---------------------------------------------------------------------------
// System status (sandbox / voice / remote-control)
// ---------------------------------------------------------------------------

const FORMAT_SANDBOX: &[&str] = &["glyph", "text", "word"];
const FORMAT_VOICE: &[&str] = &["icon", "icon-text", "text", "word"];
const FORMAT_REMOTE: &[&str] = &[
    "icon",
    "icon-text",
    "text",
    "word",
    "label-check",
    "label-mark",
];

static META_SANDBOX_STATUS: WidgetMeta = WidgetMeta {
    id: "sandbox-status",
    label: "Sandbox status",
    category: WidgetCategory::System,
    description: "Whether Claude Code is running in the sandbox profile.",
    styling: Styling::Standard,
    knobs: &[
        WidgetKnob::Meta(MetaKnob {
            key: "format",
            label: "Format",
            shape: MetaShape::Choice {
                options: FORMAT_SANDBOX,
            },
        }),
        KNOB_USE_NERD_FONT,
    ],
};

static META_VOICE_STATUS: WidgetMeta = WidgetMeta {
    id: "voice-status",
    label: "Voice status",
    category: WidgetCategory::System,
    description: "Whether the voice-dictation microphone is active.",
    styling: Styling::Standard,
    knobs: &[
        WidgetKnob::Meta(MetaKnob {
            key: "format",
            label: "Format",
            shape: MetaShape::Choice {
                options: FORMAT_VOICE,
            },
        }),
        KNOB_USE_NERD_FONT,
    ],
};

static META_REMOTE_CONTROL_STATUS: WidgetMeta = WidgetMeta {
    id: "remote-control-status",
    label: "Remote-control status",
    category: WidgetCategory::System,
    description: "Whether Claude Code's remote-control agent is attached to this session.",
    styling: Styling::Standard,
    knobs: &[
        WidgetKnob::Meta(MetaKnob {
            key: "format",
            label: "Format",
            shape: MetaShape::Choice {
                options: FORMAT_REMOTE,
            },
        }),
        KNOB_USE_NERD_FONT,
    ],
};

// ---------------------------------------------------------------------------
// Powerline / layout markers
// ---------------------------------------------------------------------------

static META_SEPARATOR: WidgetMeta = WidgetMeta {
    id: "separator",
    label: "Separator",
    category: WidgetCategory::Powerline,
    description: "The global-default separator glyph.",
    styling: Styling::Marker,
    knobs: &[],
};

static META_FLEX_SEPARATOR: WidgetMeta = WidgetMeta {
    id: "flex-separator",
    label: "Flex separator",
    category: WidgetCategory::Powerline,
    description: "Layout marker — expands to fill remaining terminal width. Layout only, no styling.",
    styling: Styling::Marker,
    knobs: &[],
};

// ---------------------------------------------------------------------------
// The METAS map.
// ---------------------------------------------------------------------------

pub static METAS: phf::Map<&'static str, &'static WidgetMeta> = phf_map! {
    "block-reset-timer" => &META_BLOCK_RESET_TIMER,
    "block-timer" => &META_BLOCK_TIMER,
    "cache-hit-rate" => &META_CACHE_HIT_RATE,
    "cache-read" => &META_CACHE_READ,
    "cache-timer" => &META_CACHE_TIMER,
    "cache-write" => &META_CACHE_WRITE,
    "claude-account-email" => &META_CLAUDE_ACCOUNT_EMAIL,
    "claude-session-id" => &META_CLAUDE_SESSION_ID,
    "compaction-counter" => &META_COMPACTION_COUNTER,
    "context-bar" => &META_CONTEXT_BAR,
    "context-length" => &META_CONTEXT_LENGTH,
    "context-percentage" => &META_CONTEXT_PERCENTAGE,
    "context-percentage-usable" => &META_CONTEXT_PERCENTAGE_USABLE,
    "context-window" => &META_CONTEXT_WINDOW,
    "current-working-dir" => &META_CWD,
    "custom-command" => &META_CUSTOM_COMMAND,
    "custom-symbol" => &META_CUSTOM_SYMBOL,
    "custom-text" => &META_CUSTOM_TEXT,
    "extra-usage-remaining" => &META_EXTRA_USAGE_REMAINING,
    "extra-usage-used" => &META_EXTRA_USAGE_USED,
    "extra-usage-utilization" => &META_EXTRA_USAGE_UTILIZATION,
    "fable-weekly-usage" => &META_FABLE_WEEKLY_USAGE,
    "flex-separator" => &META_FLEX_SEPARATOR,
    "free-memory" => &META_FREE_MEMORY,
    "git-ahead-behind" => &META_GIT_AHEAD_BEHIND,
    "git-branch" => &META_GIT_BRANCH,
    "git-changes" => &META_GIT_CHANGES,
    "git-ci-status" => &META_GIT_CI_STATUS,
    "git-clean-status" => &META_GIT_CLEAN_STATUS,
    "git-conflicts" => &META_GIT_CONFLICTS,
    "git-deletions" => &META_GIT_DELETIONS,
    "git-insertions" => &META_GIT_INSERTIONS,
    "git-is-fork" => &META_GIT_IS_FORK,
    "git-origin-host" => &META_GIT_ORIGIN_HOST,
    "git-origin-owner" => &META_GIT_ORIGIN_OWNER,
    "git-origin-owner-repo" => &META_GIT_ORIGIN_OWNER_REPO,
    "git-origin-repo" => &META_GIT_ORIGIN_REPO,
    "git-pr" => &META_GIT_PR,
    "git-root-dir" => &META_GIT_ROOT_DIR,
    "git-sha" => &META_GIT_SHA,
    "git-staged" => &META_GIT_STAGED,
    "git-staged-files" => &META_GIT_STAGED_FILES,
    "git-status" => &META_GIT_STATUS,
    "git-unstaged" => &META_GIT_UNSTAGED,
    "git-unstaged-files" => &META_GIT_UNSTAGED_FILES,
    "git-untracked" => &META_GIT_UNTRACKED,
    "git-untracked-files" => &META_GIT_UNTRACKED_FILES,
    "git-upstream-owner" => &META_GIT_UPSTREAM_OWNER,
    "git-upstream-owner-repo" => &META_GIT_UPSTREAM_OWNER_REPO,
    "git-upstream-repo" => &META_GIT_UPSTREAM_REPO,
    "git-worktree" => &META_GIT_WORKTREE,
    "git-worktree-branch" => &META_GIT_WORKTREE_BRANCH,
    "git-worktree-mode" => &META_GIT_WORKTREE_MODE,
    "git-worktree-name" => &META_GIT_WORKTREE_NAME,
    "git-worktree-original-branch" => &META_GIT_WORKTREE_ORIGINAL_BRANCH,
    "input-speed" => &META_INPUT_SPEED,
    "jj-bookmarks" => &META_JJ_BOOKMARKS,
    "jj-changes" => &META_JJ_CHANGES,
    "jj-deletions" => &META_JJ_DELETIONS,
    "jj-description" => &META_JJ_DESCRIPTION,
    "jj-insertions" => &META_JJ_INSERTIONS,
    "jj-revision" => &META_JJ_REVISION,
    "jj-root-dir" => &META_JJ_ROOT_DIR,
    "jj-workspace" => &META_JJ_WORKSPACE,
    "link" => &META_LINK,
    "model" => &META_MODEL,
    "output-speed" => &META_OUTPUT_SPEED,
    "output-style" => &META_OUTPUT_STYLE,
    "remote-control-status" => &META_REMOTE_CONTROL_STATUS,
    "sandbox-status" => &META_SANDBOX_STATUS,
    "separator" => &META_SEPARATOR,
    "session-clock" => &META_SESSION_CLOCK,
    "session-cost" => &META_SESSION_COST,
    "session-name" => &META_SESSION_NAME,
    "session-usage" => &META_SESSION_USAGE,
    "skills" => &META_SKILLS,
    "terminal-width" => &META_TERMINAL_WIDTH,
    "thinking-effort" => &META_THINKING_EFFORT,
    "tokens-cached" => &META_TOKENS_CACHED,
    "tokens-input" => &META_TOKENS_INPUT,
    "tokens-output" => &META_TOKENS_OUTPUT,
    "tokens-total" => &META_TOKENS_TOTAL,
    "total-speed" => &META_TOTAL_SPEED,
    "version" => &META_VERSION,
    "vim-mode" => &META_VIM_MODE,
    "voice-status" => &META_VOICE_STATUS,
    "weekly-opus-usage" => &META_WEEKLY_OPUS_USAGE,
    "weekly-reset-timer" => &META_WEEKLY_RESET_TIMER,
    "weekly-sonnet-usage" => &META_WEEKLY_SONNET_USAGE,
    "weekly-usage" => &META_WEEKLY_USAGE,
};
