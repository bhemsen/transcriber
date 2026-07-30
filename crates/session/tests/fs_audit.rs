//! Device-free session FS audit (spec, "Gates und Dokumente": "Der
//! Sitzungs-FS-Audit läuft in Phase 1"). Machine proof of the constitution's
//! opening promise ("Nichts Audio-förmiges wird geschrieben") for the whole
//! `session` orchestrator, using the test-tone source so it needs no audio
//! device — see `docs/specs/spec-capture-foundation.md`.
//!
//! Runs a whole session in a **child process**
//! (`env!("CARGO_BIN_EXE_fs_audit_child")`,
//! `crates/session/src/bin/fs_audit_child.rs`), because redirecting
//! `TMP`/`TEMP` in-process is not a valid route: `std::env::set_var` is
//! `unsafe` in edition 2024, and `forbid(unsafe_code)` covers test code
//! too. The child gets a fresh working directory (`Command::current_dir`)
//! and a fresh, process-private `TMP`/`TEMP` (`Command::env`) — never the
//! shared system temp root, which cargo and unrelated processes write to
//! and which would make this gate flaky by construction (spec's prior
//! decision).
//!
//! Both watched directories are recursively snapshotted once before the
//! child is spawned, polled every [`POLL_INTERVAL`] while it runs, and
//! snapshotted once more after it exits; the union of every path ever
//! observed must be empty. That union — not a single before/after pair —
//! is what gives polling *during* the run a chance to notice a file
//! written and removed again before the child exits.
//!
//! **Honest limit, not overclaimed:** polling from a separate process can
//! only notice a file that exists at the instant of a poll. A write
//! immediately followed by a delete, both landing strictly between two
//! polls, is invisible to this mechanism — no wall-clock polling scheme
//! can promise otherwise without an OS-level filesystem watch, which this
//! phase does not introduce. `catches_a_transient_write_that_outlives_the_poll_interval`
//! proves the bound this *does* meet: a transient file that outlives a
//! handful of poll intervals is caught, through a real child process, not
//! a simulation in this test's own process.
//!
//! [`detects_a_file_written_into_either_watched_directory`] proves the
//! `snapshot`/set-difference primitive itself can fail (a bug in the walk,
//! or a watch on the wrong path, would otherwise report "0 new files" for
//! the wrong reason). [`the_real_harness_notices_a_leak_from_a_real_child_process`]
//! proves the same for [`run_and_watch`] end to end — a bug that made it
//! watch the wrong directory, or never call [`snapshot`] at all, would
//! otherwise leave every other test in this file green.

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
/// `catches_a_transient_write_that_outlives_the_poll_interval`), long
/// enough that polling an otherwise-empty directory this often is cheap
/// for the whole run.
const POLL_INTERVAL: Duration = Duration::from_millis(2);

/// How long [`catches_a_transient_write_that_outlives_the_poll_interval`]
/// holds its evidence file open before deleting it — comfortably more than
/// [`POLL_INTERVAL`] to leave headroom for scheduling jitter on a loaded CI
/// runner, while still being "transient" against the child's own lifetime.
const RELIABLE_TRANSIENT_HOLD_MS: u64 = 50;

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
fn fresh_dir(label: &str) -> PathBuf {
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
fn cleanup(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}

/// Every file under `root`, as paths relative to it — recursing into every
/// subdirectory, not just its top level, so a file written into a nested
/// directory is not missed.
fn snapshot(root: &Path) -> BTreeSet<PathBuf> {
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
/// harness-level detector tests to flip on [`fs_audit_child`]'s env-gated
/// leak or transient-write simulation without duplicating the spawn logic.
/// Panicking here would otherwise leak `work_dir`/`temp_dir`, so both are
/// removed first.
fn spawn_child(work_dir: &Path, temp_dir: &Path, extra_env: &[(&str, &str)]) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fs_audit_child"));
    command
        .current_dir(work_dir)
        .env("TMP", temp_dir)
        .env("TEMP", temp_dir)
        .env("TMPDIR", temp_dir)
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
fn run_and_watch(
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
/// exited. A non-zero exit status alone would not catch a session that
/// silently failed to start: that would also leave 0 new files behind and
/// pass the directory check vacuously.
fn assert_frames_captured(stdout: &str, key: &str) {
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

#[test]
fn a_whole_session_leaves_no_new_file_in_its_working_or_temp_directory() {
    let work_dir = fresh_dir("work");
    let temp_dir = fresh_dir("tmp");

    let (status, stdout, stderr, observed) = run_and_watch(&work_dir, &temp_dir, &[]);

    cleanup(&work_dir);
    cleanup(&temp_dir);

    assert!(
        status.success(),
        "fs_audit_child exited with {status}, stderr:\n{stderr}"
    );
    assert!(
        observed.is_empty(),
        "session left new files behind: {observed:?}"
    );
    assert_frames_captured(&stdout, "remote_frames");
    assert_frames_captured(&stdout, "local_frames");
}

/// Proves the `snapshot`/set-difference primitive can fail: without this,
/// an audit that always reports "0 new files" — because of a bug in the
/// recursive walk, or because it never looked at the right directory —
/// would pass for the wrong reason. Covers both watched directory kinds
/// independently, and writes into a *nested* subdirectory of the working
/// directory, so a non-recursive walk would also be caught. Deliberately
/// exercises `snapshot` directly rather than going through
/// [`run_and_watch`] — see [`the_real_harness_notices_a_leak_from_a_real_child_process`]
/// for the end-to-end version of this same proof.
#[test]
fn detects_a_file_written_into_either_watched_directory() {
    let work_dir = fresh_dir("detector-work");
    let temp_dir = fresh_dir("detector-tmp");

    let before_work = snapshot(&work_dir);
    let before_temp = snapshot(&temp_dir);
    assert!(
        before_work.is_empty(),
        "sanity: a fresh directory starts empty"
    );
    assert!(
        before_temp.is_empty(),
        "sanity: a fresh directory starts empty"
    );

    let nested = work_dir.join("nested").join("evidence.txt");
    let nested_parent = nested.parent().unwrap_or(&work_dir);
    let Ok(()) = fs::create_dir_all(nested_parent) else {
        panic!("failed to create a nested directory under the watched work dir");
    };
    let Ok(()) = fs::write(&nested, b"not audio, just evidence the detector works") else {
        panic!("failed to write the detector's evidence file in the work dir");
    };
    let Ok(()) = fs::write(temp_dir.join("evidence.txt"), b"same, in the temp dir") else {
        panic!("failed to write the detector's evidence file in the temp dir");
    };

    let after_work = snapshot(&work_dir);
    let after_temp = snapshot(&temp_dir);

    cleanup(&work_dir);
    cleanup(&temp_dir);

    let new_in_work: Vec<_> = after_work.difference(&before_work).collect();
    let new_in_temp: Vec<_> = after_temp.difference(&before_temp).collect();

    assert_eq!(
        new_in_work.len(),
        1,
        "expected exactly the nested evidence file, got {new_in_work:?}"
    );
    assert!(new_in_work[0].ends_with("evidence.txt"));
    assert_eq!(
        new_in_temp.len(),
        1,
        "expected exactly the evidence file, got {new_in_temp:?}"
    );
}

/// Proves [`run_and_watch`] itself — the exact machinery
/// `a_whole_session_leaves_no_new_file_in_its_working_or_temp_directory`
/// relies on — actually notices a leak from a *real* child process, not
/// just from the pure `snapshot`/set-difference primitive above. Without
/// this, a bug that made `run_and_watch` watch the wrong directory, or
/// never call `snapshot` at all, could leave every other test in this
/// file green. `fs_audit_child` only leaves this evidence behind when
/// `FS_AUDIT_CHILD_LEAK_EVIDENCE` is set — never during the main audit
/// test.
#[test]
fn the_real_harness_notices_a_leak_from_a_real_child_process() {
    let work_dir = fresh_dir("harness-leak-work");
    let temp_dir = fresh_dir("harness-leak-tmp");

    let (status, _stdout, stderr, observed) = run_and_watch(
        &work_dir,
        &temp_dir,
        &[("FS_AUDIT_CHILD_LEAK_EVIDENCE", "1")],
    );

    cleanup(&work_dir);
    cleanup(&temp_dir);

    assert!(
        status.success(),
        "fs_audit_child exited with {status}, stderr:\n{stderr}"
    );
    assert!(
        observed
            .iter()
            .any(|path| path.ends_with("leaked-into-cwd.txt")),
        "run_and_watch did not notice the working-directory leak: {observed:?}"
    );
    assert!(
        observed
            .iter()
            .any(|path| path.ends_with("leaked-into-tmp.txt")),
        "run_and_watch did not notice the temp-directory leak: {observed:?}"
    );
}

/// The honest half of the write-then-delete story (see the module doc's
/// "Honest limit" section): a transient file that outlives a handful of
/// [`POLL_INTERVAL`]s *is* caught, proven end to end through a real child
/// process and the exact `run_and_watch` machinery the main audit test
/// relies on — not simulated inside this test's own process. This does
/// *not* prove every write-then-delete is caught regardless of duration;
/// no wall-clock poll from a separate process can promise that.
#[test]
fn catches_a_transient_write_that_outlives_the_poll_interval() {
    let work_dir = fresh_dir("transient-work");
    let temp_dir = fresh_dir("transient-tmp");
    let hold_ms = RELIABLE_TRANSIENT_HOLD_MS.to_string();

    let (status, _stdout, stderr, observed) = run_and_watch(
        &work_dir,
        &temp_dir,
        &[("FS_AUDIT_CHILD_TRANSIENT_WRITE_MS", hold_ms.as_str())],
    );

    cleanup(&work_dir);
    cleanup(&temp_dir);

    assert!(
        status.success(),
        "fs_audit_child exited with {status}, stderr:\n{stderr}"
    );
    assert!(
        observed
            .iter()
            .any(|path| path.ends_with("transient-evidence.txt")),
        "a transient write held for {hold_ms}ms was not observed: {observed:?}"
    );
}
