//! The pins of slice 7, the git reading bound, over stand-in
//! git executables and synthetic directories the suite writes
//! itself. The pins that move the path are serialized on the
//! path they share.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use catchledger::ingest::{read_git_history_from_clone, GIT_TIME_LIMIT};

static PATH_LOCK: Mutex<()> = Mutex::new(());

const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "catchledger-git-timeout-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch directory stands");
    dir
}

/// Write a stand-in git executable carrying the given shell body
/// into its own directory under the scratch directory and return
/// that directory, to stand first on the path.
fn stand_in_git(dir: &Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).expect("the stand-in directory stands");
    let script = bin.join("git");
    std::fs::write(&script, body).expect("the stand-in writes");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("the stand-in stands executable");
    bin
}

/// Run the closure with the stand-in directory first on the
/// path, restoring the path it found afterwards.
fn with_stand_in_path<T>(bin: &Path, run: impl FnOnce() -> T) -> T {
    let original = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin.to_path_buf()];
    paths.extend(std::env::split_paths(&original));
    let joined = std::env::join_paths(paths).expect("the path joins");
    std::env::set_var("PATH", &joined);
    let outcome = run();
    std::env::set_var("PATH", original);
    outcome
}

#[test]
fn test_a_git_that_does_not_finish_is_refused_within_the_bound() {
    let _guard = PATH_LOCK.lock().expect("the path lock stands");
    let dir = scratch("hanging");
    let bin = stand_in_git(&dir, "#!/bin/sh\nsleep 30\n");
    let started = Instant::now();
    let error = with_stand_in_path(&bin, || {
        read_git_history_from_clone(&dir, Duration::from_secs(1))
            .expect_err("a git that never finishes refuses")
    });
    assert_eq!(
        error.message(),
        "the local git did not finish within the allowed time"
    );
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "the refusal stands at the bound and not at the stand-in's sleep"
    );
}

#[test]
fn test_a_slow_git_inside_the_bound_is_read_wholly() {
    let _guard = PATH_LOCK.lock().expect("the path lock stands");
    let dir = scratch("slow");
    let body = format!(
        "#!/bin/sh\nsleep 1\nprintf 'commit {HASH_A}\\nparents \\ndate 2026-01-01T00:00:00Z\\nsubject Synthetic commit\\nA\\tf.txt\\n'\n"
    );
    let bin = stand_in_git(&dir, &body);
    let (records, _digest) = with_stand_in_path(&bin, || {
        read_git_history_from_clone(&dir, Duration::from_secs(30))
            .expect("a slow git inside the bound reads")
    });
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].changes.len(), 1);
    assert_eq!(records[0].changes[0].paths, vec!["f.txt".to_string()]);
}

#[test]
fn test_a_failing_git_keeps_its_standing_refusal_under_the_bound() {
    let _guard = PATH_LOCK.lock().expect("the path lock stands");
    let dir = scratch("failing");
    let bin = stand_in_git(&dir, "#!/bin/sh\nexit 1\n");
    let error = with_stand_in_path(&bin, || {
        read_git_history_from_clone(&dir, Duration::from_secs(30))
            .expect_err("a failing git refuses")
    });
    assert_eq!(
        error.message(),
        "the local Git repository could not be read"
    );
}

#[test]
fn test_the_git_time_limit_stands_at_120_seconds() {
    assert_eq!(GIT_TIME_LIMIT, Duration::from_secs(120));
}
