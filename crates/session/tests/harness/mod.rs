//! Shared machinery behind every test in `../fs_audit.rs`: creating and
//! removing scratch directories, recursively snapshotting them, and
//! spawning-and-watching `fs_audit_child` while it runs. Split out purely
//! to keep `fs_audit.rs` itself under the constitution's 400-line module
//! limit — the same reason `xtask/tests/no_write_paths.rs` keeps its own
//! detectors in `audit/mod.rs` rather than inline. Lives in its own
//! `harness/mod.rs` subdirectory (not a bare `harness.rs` directly under
//! `tests/`) so Cargo does not discover it as a second, independent test
//! target — the same reasoning the spec's Decision log records for
//! `xtask/tests/audit/mod.rs`.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use std::{env, fs};

/// How often [`run_and_watch`] re-snapshots both watched directories while
/// the child is alive. Short enough that a transient file needs to survive
/// only a handful of these to be caught (see
/// `catches_a_transient_write_that_outlives_the_poll_interval` in
/// `../fs_audit.rs`), long enough that polling an otherwise-empty
/// directory this often is cheap for the whole run.
pub(crate) const POLL_INTERVAL: Duration = Duration::from_millis(2);

/// How long `catches_a_transient_write_that_outlives_the_poll_interval`
/// (`../fs_audit.rs`) holds its evidence file open before deleting it —
/// comfortably more than [`POLL_INTERVAL`] to leave headroom for
/// scheduling jitter on a loaded CI runner, while still being "transient"
/// against the child's own lifetime.
pub(crate) const RELIABLE_TRANSIENT_HOLD_MS: u64 = 50;

/// Per-process counter so two scratch directories created in the same test
/// run — even from parallel test threads — never collide on name alone.
static DIR_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Creates and returns a fresh, empty directory under the shared system
/// temp root, named so nothing else in the system could guess it. From this
/// point on the directory is process-private in the sense that matters
/// here: only this test knows its path, watches it, or removes it — never
/// the shared root it lives under. Uses `create_dir` rather than
/// `create_dir_all`: a name collision (e.g. a leaked directory from a
/// crashed prior run reusing a recycled PID) should be a loud error here,
/// not a silent reuse of a directory that might not actually be empty.
pub(crate) fn fresh_dir(label: &str) -> PathBuf {
    let sequence = DIR_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let name = format!(
        "transcriber-fs-audit-{}-{label}-{sequence}",
        std::process::id()
    );
    let dir = env::temp_dir().join(name);
    let Ok(()) = fs::create_dir(&dir) else {
        panic!(
            "failed to create a fresh scratch directory at {}",
            dir.display()
        );
    };
    dir
}

/// Removes a scratch directory this test created. Never called on anything
/// but a path [`fresh_dir`] itself returned.
pub(crate) fn cleanup(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}

/// Every file under `root`, as paths relative to it — recursing into every
/// subdirectory, not just its top level, so a file written into a nested
/// directory is not missed. Treats an unreadable or already-gone `root` the
/// same as an empty one (via [`collect_files`]'s `let ... else { return }`)
/// rather than surfacing the read error — acceptable here because every
/// caller only ever calls this on a directory it just created itself and
/// still owns, never one it suspects may have vanished.
pub(crate) fn snapshot(root: &Path) -> BTreeSet<PathBuf> {
    let mut files = BTreeSet::new();
    collect_files(root, root, &mut files);
    files
}

fn collect_files(root: &Path, dir: &Path, files: &mut BTreeSet<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files);
        } else if let Ok(relative) = path.strip_prefix(root) {
            files.insert(relative.to_path_buf());
        }
    }
}

/// Spawns the child with a fresh working directory, a fresh process-private
/// `TMP`/`TEMP`/`TMPDIR`, and any `extra_env` on top — used by the
/// harness-level detector tests to flip on `fs_audit_child`'s env-gated
/// leak or transient-write simulation without duplicating the spawn logic.
/// Panicking here would otherwise leak `work_dir`/`temp_dir`, so both are
/// removed first.
///
/// Explicitly removes both detector env vars before applying `extra_env`:
/// `Command::env` only adds to the *inherited* environment, so without
/// this, either var happening to already be set in the calling shell would
/// leak into the main audit test's otherwise-plain child. That direction
/// is not silently unsafe — it would only make the main test fail louder,
/// never pass when it should not — but there is no reason to depend on it.
fn spawn_child(work_dir: &Path, temp_dir: &Path, extra_env: &[(&str, &str)]) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fs_audit_child"));
    command
        .current_dir(work_dir)
        .env("TMP", temp_dir)
        .env("TEMP", temp_dir)
        .env("TMPDIR", temp_dir)
        .env_remove("FS_AUDIT_CHILD_LEAK_EVIDENCE")
        .env_remove("FS_AUDIT_CHILD_TRANSIENT_WRITE_MS")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in extra_env {
        command.env(key, value);
    }
    match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            cleanup(work_dir);
            cleanup(temp_dir);
            panic!("failed to spawn fs_audit_child: {error}");
        }
    }
}

fn read_all(pipe: Option<impl Read>) -> String {
    let mut buffer = String::new();
    if let Some(mut pipe) = pipe {
        let _ = pipe.read_to_string(&mut buffer);
    }
    buffer
}

/// Waits for `child` to exit, polling both watched directories every
/// [`POLL_INTERVAL`] the whole time — plus once right before spawning it
/// and once right after it exits. Returns every path observed at any of
/// those points; empty on a clean pass. Kills `child` and fails loudly
/// past a 10 s deadline rather than hanging a CI job forever.
fn poll_until_exit(
    child: &mut Child,
    work_dir: &Path,
    temp_dir: &Path,
    observed: &mut BTreeSet<PathBuf>,
) -> ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        observed.extend(snapshot(work_dir));
        observed.extend(snapshot(temp_dir));
        match child.try_wait() {
            Ok(Some(status)) => return status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(POLL_INTERVAL);
            }
            Ok(None) => {
                let _ = child.kill();
                cleanup(work_dir);
                cleanup(temp_dir);
                panic!("fs_audit_child did not exit within the deadline");
            }
            Err(error) => {
                cleanup(work_dir);
                cleanup(temp_dir);
                panic!("failed to poll fs_audit_child: {error}");
            }
        }
    }
}

/// Runs the child (with `extra_env` on top of the usual redirected
/// directories) to completion. Returns its exit status, its captured
/// stdout and stderr, and every path observed in either watched directory
/// across the whole run, from before it was spawned to after it exited.
pub(crate) fn run_and_watch(
    work_dir: &Path,
    temp_dir: &Path,
    extra_env: &[(&str, &str)],
) -> (ExitStatus, String, String, BTreeSet<PathBuf>) {
    let mut observed = BTreeSet::new();
    observed.extend(snapshot(work_dir));
    observed.extend(snapshot(temp_dir));

    let mut child = spawn_child(work_dir, temp_dir, extra_env);
    let status = poll_until_exit(&mut child, work_dir, temp_dir, &mut observed);
    observed.extend(snapshot(work_dir));
    observed.extend(snapshot(temp_dir));

    let stdout = read_all(child.stdout.take());
    let stderr = read_all(child.stderr.take());
    (status, stdout, stderr, observed)
}

/// Fails unless `stdout` contains a `{key}=<n>` line with `n > 0` — the
/// proof the child's session actually pulled frames, not just started and
/// exited. Checking only the exit status would not catch this: a session
/// that starts fine but silently pulls nothing still exits 0, would still
/// leave 0 new files behind, and would pass the directory check
/// vacuously.
pub(crate) fn assert_frames_captured(stdout: &str, key: &str) {
    let Some(line) = stdout.lines().find(|line| line.starts_with(key)) else {
        panic!("fs_audit_child stdout missing a {key} line:\n{stdout}");
    };
    let Some((_, value)) = line.split_once('=') else {
        panic!("malformed {key} line: {line}");
    };
    let Ok(count) = value.trim().parse::<u64>() else {
        panic!("non-numeric {key} value: {value}");
    };
    assert!(count > 0, "{key} was {count}: the session captured nothing");
}
