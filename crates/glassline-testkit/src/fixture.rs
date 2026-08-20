//! Load recorded StatusJSON payloads from `test-fixtures/status-json/`.
//!
//! Every fixture is a `.json` file whose stem becomes its ID. Tests iterate
//! [`FixtureLoader`] to run the same assertion across every recorded case.

use std::{
    fs,
    path::{Path, PathBuf},
};

use glassline_core::status_json::StatusJson;
use thiserror::Error;

/// One recorded payload.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// Stem of the source file (`minimal.json` → `"minimal"`).
    pub name: String,
    /// Absolute path on disk.
    pub path: PathBuf,
    /// Raw JSON string.
    pub raw: String,
}

impl Fixture {
    /// Parse the raw JSON as [`StatusJson`].
    pub fn parse(&self) -> Result<StatusJson, FixtureError> {
        serde_json::from_str(&self.raw).map_err(FixtureError::from)
    }
}

/// Walk a directory of `.json` fixtures.
#[derive(Debug, Clone)]
pub struct FixtureLoader {
    root: PathBuf,
}

impl FixtureLoader {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Locate the workspace's default status-json fixture directory
    /// (`<workspace-root>/test-fixtures/status-json`). Panics when the tree
    /// layout is missing — this is only used from tests.
    #[must_use]
    pub fn default_status_json() -> Self {
        let root = workspace_root().join("test-fixtures").join("status-json");
        Self::new(root)
    }

    /// Enumerate every `.json` file under [`Self::root`].
    pub fn iter(&self) -> Result<Vec<Fixture>, FixtureError> {
        collect_fixtures(&self.root)
    }
}

fn collect_fixtures(root: &Path) -> Result<Vec<Fixture>, FixtureError> {
    let mut out = Vec::new();
    if !root.exists() {
        return Ok(out);
    }
    let entries = fs::read_dir(root).map_err(|e| FixtureError::Io {
        path: root.to_path_buf(),
        source: e,
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| FixtureError::Io {
            path: root.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("<unknown>")
            .to_string();
        let raw = fs::read_to_string(&path).map_err(|e| FixtureError::Io {
            path: path.clone(),
            source: e,
        })?;
        out.push(Fixture { name, path, raw });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Best-effort workspace-root discovery. Walks up from `CARGO_MANIFEST_DIR`
/// until a `Cargo.toml` with `[workspace]` shows up.
fn workspace_root() -> PathBuf {
    let start = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    for ancestor in start.ancestors() {
        let manifest = ancestor.join("Cargo.toml");
        if !manifest.exists() {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&manifest)
            && text.contains("[workspace]")
        {
            return ancestor.to_path_buf();
        }
    }
    start
}

#[derive(Debug, Error)]
pub enum FixtureError {
    #[error("failed reading {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed parsing StatusJson fixture: {0}")]
    Parse(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_loader_finds_fixtures() {
        let loader = FixtureLoader::default_status_json();
        let fixtures = loader.iter().expect("read fixtures dir");
        assert!(
            fixtures.len() >= 5,
            "expected at least 5 fixtures under test-fixtures/status-json, got {}",
            fixtures.len()
        );
    }

    #[test]
    fn every_fixture_parses_as_status_json() {
        let loader = FixtureLoader::default_status_json();
        for fixture in loader.iter().unwrap() {
            fixture
                .parse()
                .unwrap_or_else(|e| panic!("fixture {} failed to parse: {e}", fixture.name));
        }
    }
}
