//! The record of one staged update: `update.json` in the data folder.
//!
//! Sayso writes each phase before the action of that phase, so a start
//! after a crash knows how far the update came.

use crate::Failure;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// The new version is beside the installed app and waits for the swap.
    Staged,
    /// The swap started. The exchange happened or it did not.
    Swapping,
    /// The exchange happened.
    Swapped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// The complete `latest.json` that the staged update came from. Sayso
    /// verifies its signature again before the swap.
    pub envelope: String,
    pub from_version: Version,
    pub to_version: Version,
    /// The installed app at the time of staging.
    pub install_path: PathBuf,
    pub phase: Phase,
    /// The number of swaps that started.
    pub attempts: u32,
    /// The English text of the failure, for support.
    pub error: Option<String>,
    /// The failure itself. A record from an older Sayso has only `error`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<Failure>,
}

impl Record {
    pub fn load(path: &Path) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        serde_json::from_slice(&bytes).inspect_err(|e| log::warn!("update: {} is not readable: {e}", path.display())).ok()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        crate::files::write_atomic(path, &serde_json::to_vec_pretty(self)?)
    }

    pub fn delete(path: &Path) {
        let _ = std::fs::remove_file(path);
    }

    /// Write the phase `failed` with its cause.
    pub fn fail(mut self, path: &Path, failure: Failure) -> Self {
        self.phase = Phase::Failed;
        self.error = Some(failure.to_string());
        self.failure = Some(failure);
        if let Err(e) = self.save(path) {
            log::error!("update: could not write {}: {e}", path.display());
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> Record {
        Record {
            envelope: String::new(),
            from_version: Version::new(0, 1, 0),
            to_version: Version::new(0, 2, 0),
            install_path: PathBuf::from("/Applications/Sayso.app"),
            phase: Phase::Staged,
            attempts: 1,
            error: None,
            failure: None,
        }
    }

    #[test]
    fn a_failure_keeps_its_kind_and_its_english_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update.json");
        record().fail(&path, Failure::Replayed(Version::new(0, 1, 5)));
        let back = Record::load(&path).unwrap();
        assert_eq!(back.failure, Some(Failure::Replayed(Version::new(0, 1, 5))));
        assert!(back.error.unwrap().contains("offered version 0.1.5"));
    }

    #[test]
    fn a_record_of_an_older_sayso_has_no_kind() {
        let mut old = serde_json::to_value(record()).unwrap();
        old.as_object_mut().unwrap().remove("failure");
        old["error"] = "The update failed in some way.".into();
        let back: Record = serde_json::from_value(old).unwrap();
        assert_eq!(back.failure, None);
        assert_eq!(back.error.as_deref(), Some("The update failed in some way."));
    }
}
