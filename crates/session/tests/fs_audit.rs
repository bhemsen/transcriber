//! Device-free session FS audit (spec, "Gates und Dokumente": "Der
//! Sitzungs-FS-Audit läuft in Phase 1"). Machine proof of the constitution's
//! opening promise ("Nichts Audio-förmiges wird geschrieben") for a whole
//! `session` run's **working directory and its redirected temp
//! directory**, using the test-tone source so it needs no audio device —
//! see `docs/specs/spec-capture-foundation.md`. Deliberately scoped to
//! those two directories, not the whole filesystem: the spec's own
//! rationale for that scope is that snapshotting the shared user profile
//! would be exactly as flaky as snapshotting the shared temp root (cargo
//! and unrelated processes write there too). A write to an absolute path
//! outside both — e.g. `%LOCALAPPDATA%` — is invisible to this audit; see
//! the second "Honest limit" below. `session` is not (yet) in
//! `xtask::audit::GUARDED_CRATES`, so there is no symbol-level backstop for
//! that gap either — an open question the spec's Decision log carries to
//! planning, not one this test resolves.
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
//! child is spawned, polled every [`harness::POLL_INTERVAL`] while it
//! runs, and snapshotted once more after it exits (all in
//! [`harness::run_and_watch`]); the union of every path ever observed must
//! be empty. That union — not a single before/after pair — is what gives
//! polling *during* the run a chance to notice a file written and removed
//! again before the child exits.
//!
//! **Honest limit #1, not overclaimed:** polling from a separate process
//! can only notice a file that exists at the instant of a poll. A write
//! immediately followed by a delete, both landing strictly between two
//! polls, is invisible to this mechanism — no wall-clock polling scheme
//! can promise otherwise without an OS-level filesystem watch, which this
//! phase does not introduce. [`catches_a_transient_write_that_outlives_the_poll_interval`]
//! proves the bound this *does* meet: a transient file that outlives a
//! handful of poll intervals is caught, through a real child process, not
//! a simulation in this test's own process. [`harness::POLL_INTERVAL`]
//! (2 ms) is what makes that bound tight — a review round measured that
//! raising `fs_audit_child`'s `TOTAL_FRAMES` contributes nothing to
//! catching a transient write on its own (the write happens on the
//! child's main thread, decoupled from the capture threads' workload);
//! see the spec's Decision log for the counterfactual and the
//! platform-dependent reason 2 ms catches what 15 ms did not.
//!
//! **Honest limit #2:** this audit only watches the two directories named
//! above. A write to any other absolute path is invisible to it — see the
//! module doc's opening paragraph.
//!
//! [`detects_a_file_written_into_either_watched_directory`] proves the
//! `snapshot`/set-difference primitive itself can fail (a bug in the walk,
//! or a watch on the wrong path, would otherwise report "0 new files" for
//! the wrong reason). [`the_real_harness_notices_a_leak_from_a_real_child_process`]
//! proves the same for [`harness::run_and_watch`] end to end — a bug that
//! made it watch the wrong directory, or never call `snapshot` at all,
//! would otherwise leave every other test in this file green.

mod harness;

use harness::{
    RELIABLE_TRANSIENT_HOLD_MS, assert_frames_captured, cleanup, fresh_dir, run_and_watch, snapshot,
};

#[test]
fn a_whole_session_leaves_no_new_file_in_its_working_or_temp_directory() {
    let work_dir = fresh_dir("work");
    let temp_dir = fresh_dir("tmp");
    assert!(
        snapshot(&work_dir).is_empty(),
        "sanity: a fresh directory starts empty"
    );
    assert!(
        snapshot(&temp_dir).is_empty(),
        "sanity: a fresh directory starts empty"
    );

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
/// [`harness::run_and_watch`] — see
/// [`the_real_harness_notices_a_leak_from_a_real_child_process`] for the
/// end-to-end version of this same proof.
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
    let Ok(()) = std::fs::create_dir_all(nested_parent) else {
        panic!("failed to create a nested directory under the watched work dir");
    };
    let Ok(()) = std::fs::write(&nested, b"not audio, just evidence the detector works") else {
        panic!("failed to write the detector's evidence file in the work dir");
    };
    let Ok(()) = std::fs::write(temp_dir.join("evidence.txt"), b"same, in the temp dir") else {
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

/// Proves [`harness::run_and_watch`] itself — the exact machinery
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
/// [`harness::POLL_INTERVAL`]s *is* caught, proven end to end through a
/// real child process and the exact `run_and_watch` machinery the main
/// audit test relies on — not simulated inside this test's own process.
/// This does *not* prove every write-then-delete is caught regardless of
/// duration; no wall-clock poll from a separate process can promise that.
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
