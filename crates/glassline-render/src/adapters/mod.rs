//! Per-CLI adapter implementations. Each concrete adapter is a
//! `pub struct` implementing [`crate::adapter::CliAdapter`] and lives
//! in its own module — `claude.rs` for the reference implementation,
//! `codex.rs` for OpenAI Codex, `grok.rs` for xAI Grok CLI.
//!
//! Adapters are registered in [`crate::adapter::REGISTRY`], which is
//! the entry point for `glassline install --for <slug>` dispatch and
//! for `env_var_dispatch()` (chooses which adapter parses the render
//! binary's stdin based on which CLI's home env var is present).

pub mod codex;
pub mod grok;
