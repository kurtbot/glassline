//! Individual widget implementations, one per file. Each widget exposes a
//! `factory()` fn wired into [`crate::registry::WIDGETS`].

pub mod block_reset_timer;
pub mod block_timer;
pub mod claude_session_id;
pub mod compaction_counter;
pub mod context_bar;
pub mod context_length;
pub mod context_percentage;
pub mod custom_text;
pub mod cwd;
pub mod git_branch;
pub mod git_changes;
pub mod git_root_dir;
pub mod git_sha;
pub mod git_status;
pub mod link;
pub mod model;
pub mod output_style;
pub mod separator;
pub mod session_clock;
pub mod session_cost;
pub mod session_name;
pub mod speed;
pub mod thinking_effort;
pub mod tokens_input;
pub mod tokens_output;
pub mod usage;
pub mod version;
