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
//! Both watched directories are recursively snapshotted before the child
//! runs, polled while it runs, and snapshotted once more after it exits.
//! Any path observed at any of those points is a failure: polling *during*
//! the run, not only before/after, is what would still catch a file
//! written and then deleted before the process exits.
//!
//! [`detects_a_file_written_into_either_watched_directory`] is the
//! companion proof that the comparison itself can fail: without it, an
//! audit that always reports "0 new files" — because of a bug in the walk,
//! or because it never looked at the right directory — would pass for the
//! wrong reason.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use std::{env, fs};

/// Per-process counter so two scratch directories created in the same test
/// run — even from parallel test threads — never collide on name alone.
static DIR_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Creates and returns a fresh, empty directory under the shared system
/// temp root, named so nothing else in the system could guess it. From this
/// point on the directory is process-private in the sense that matters
/// here: only this test knows its path, watches it, or removes it — never
/// the shared root it lives under.
fn fresh_dir(label: &str) -> PathBuf {
    let sequence = DIR_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let name = format!("transcriber-fs-audit-{}-{label}-{sequence}", process_id());
    let dir = env::temp_dir().join(name);
    let Ok(()) = fs::create_dir_all(&dir) else {
        panic!(
            "failed to create a fresh scratch directory at {}",
            dir.display()
        );
    };
    dir
}

/// `std::process::id()`, split out only so [`fresh_dir`] reads as one line.
fn process_id() -> u32 {
    std::process::id()
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

/// Spawns the child with a fresh working directory and a fresh,
/// process-private `TMP`/`TEMP` (`TMPDIR` too, so the same test would still
/// be meaningful if this crate is ever built on a non-Windows target).
fn spawn_child(work_dir: &Path, temp_dir: &Path) -> Child {
    let Ok(child) = Command::new(env!("CARGO_BIN_EXE_fs_audit_child"))
        .current_dir(work_dir)
        .env("TMP", temp_dir)
        .env("TEMP", temp_dir)
        .env("TMPDIR", temp_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    else {
        panic!("failed to spawn fs_audit_child");
    };
    child
}

fn read_all(pipe: Option<impl Read>) -> String {
    let mut buffer = String::new();
    if let Some(mut pipe) = pipe {
        let _ = pipe.read_to_string(&mut buffer);
    }
    buffer
}

/// Runs the child to completion, polling both watched directories the whole
/// time it is alive plus once more right after it exits. Returns its exit
/// status, its captured stdout and stderr, and every path observed in
/// either directory across the whole run — empty on a clean pass.
fn run_and_watch(
    work_dir: &Path,
    temp_dir: &Path,
) -> (ExitStatus, String, String, BTreeSet<PathBuf>) {
    let mut child = spawn_child(work_dir, temp_dir);
    let mut observed = BTreeSet::new();
    let deadline = Instant::now() + Duration::from_secs(10);

    let status = loop {
        observed.extend(snapshot(work_dir));
        observed.extend(snapshot(temp_dir));
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(15));
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
    };
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

    let (status, stdout, stderr, observed) = run_and_watch(&work_dir, &temp_dir);

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

/// Proves the detector can fail: without this, an audit that always reports
/// "0 new files" — a bug in the recursive walk, or a watch on the wrong
/// path — would pass for the wrong reason. Covers both watched directory
/// kinds independently, and writes into a *nested* subdirectory of the
/// working directory, so a non-recursive walk would also be caught.
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
