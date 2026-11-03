//! Typed records used between ingestion and ledger building.
//!
//! Timestamps are UTC microseconds since the Unix epoch, as read by
//! the timestamp reading in the dates module.

/// One completed workflow run, mirroring the Python `RunRecord`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRecord {
    pub identifier: String,
    pub check: String,
    pub commit: String,
    pub conclusion: String,
    pub created_at: i128,
    pub started_at: i128,
    pub updated_at: i128,
}

impl RunRecord {
    /// The whole second distance from start to update, with a
    /// nonpositive distance contributing zero.
    pub fn duration_seconds(&self) -> i64 {
        let distance = self.updated_at - self.started_at;
        if distance <= 0 {
            0
        } else {
            (distance / 1_000_000) as i64
        }
    }
}

/// One changed path group of a commit, mirroring `CommitChange`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitChange {
    pub status: String,
    pub paths: Vec<String>,
}

/// One commit in the Git history, mirroring `CommitRecord`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRecord {
    pub commit: String,
    pub parents: Vec<String>,
    pub committed_at: i128,
    pub subject: String,
    pub changes: Vec<CommitChange>,
}

impl CommitRecord {
    /// Every changed path in first seen order, duplicates removed.
    pub fn changed_paths(&self) -> Vec<String> {
        let mut paths: Vec<String> = Vec::new();
        for change in &self.changes {
            for path in &change.paths {
                if !paths.contains(path) {
                    paths.push(path.clone());
                }
            }
        }
        paths
    }
}
