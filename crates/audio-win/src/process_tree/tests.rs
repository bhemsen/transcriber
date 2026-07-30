//! Device-free logic tests for [`super::resolve_tree_root`] and
//! [`super::identity_still_matches`] — split into their own file so the
//! shipped code in `process_tree.rs` stays well under the constitution's
//! 400-line guideline (`docs/constitution.md`, Architecture principles).

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use transcriber_core::CaptureSubject;

use super::{ProcessSnapshot, identity_still_matches, resolve_tree_root};

/// Builds a minimal process-table entry: only what [`resolve_tree_root`]
/// reads, with an arbitrary but fixed start time so tests stay
/// deterministic.
fn snapshot(name: &str, parent_pid: Option<u32>) -> ProcessSnapshot {
    ProcessSnapshot {
        name: name.to_string(),
        parent_pid,
        started_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1_000),
    }
}

#[test]
fn walk_climbs_past_a_rendering_child_to_the_real_application_root() {
    let processes = HashMap::from([
        (50, snapshot("explorer.exe", None)),
        (100, snapshot("teams.exe", Some(50))),
        (150, snapshot("teams.exe", Some(100))),
    ]);
    assert_eq!(resolve_tree_root(150, &processes), 100);
}

#[test]
fn walk_never_crosses_into_a_stop_listed_parent_name() {
    for stop_name in super::STOP_PROCESS_NAMES {
        let processes = HashMap::from([
            (50, snapshot(stop_name, None)),
            (100, snapshot("app.exe", Some(50))),
        ]);
        assert_eq!(
            resolve_tree_root(100, &processes),
            100,
            "walk must stop before climbing into {stop_name}"
        );
    }
}

#[test]
fn walk_never_crosses_into_a_stop_listed_parent_pid() {
    for &stop_pid in super::STOP_PIDS {
        // The stop PID gets its own record here deliberately: without one,
        // the orphaned-parent branch alone would already stop the walk at
        // 200, and the test would pass even if the PID stop-list check in
        // `is_stop_listed` were deleted. With a record present, only the
        // PID check itself can still stop the walk before it climbs onto
        // `stop_pid`.
        let processes = HashMap::from([
            (stop_pid, snapshot("system", None)),
            (200, snapshot("svchost-hosted.exe", Some(stop_pid))),
        ]);
        assert_eq!(resolve_tree_root(200, &processes), 200);
    }
}

#[test]
fn walk_terminates_on_a_cycle_instead_of_looping_forever() {
    let processes = HashMap::from([
        (10, snapshot("a.exe", Some(20))),
        (20, snapshot("b.exe", Some(10))),
    ]);
    // Must return, not hang — the assertion below only runs if it does.
    assert_eq!(resolve_tree_root(10, &processes), 20);
}

#[test]
fn walk_stops_at_an_orphaned_parent_with_no_process_record() {
    let processes = HashMap::from([(300, snapshot("app.exe", Some(9_999)))]);
    assert_eq!(resolve_tree_root(300, &processes), 300);
}

#[test]
fn walk_returns_the_pid_unchanged_when_it_has_no_process_record_at_all() {
    let processes = HashMap::new();
    assert_eq!(resolve_tree_root(555, &processes), 555);
}

#[test]
fn walk_stops_at_a_process_with_no_parent() {
    let processes = HashMap::from([(400, snapshot("app.exe", None))]);
    assert_eq!(resolve_tree_root(400, &processes), 400);
}

#[test]
fn identity_matches_when_name_and_start_time_are_unchanged() {
    let started_at = SystemTime::UNIX_EPOCH + Duration::from_secs(500);
    let subject = CaptureSubject::new("teams.exe", 100, started_at);
    assert!(identity_still_matches(&subject, "teams.exe", started_at));
}

#[test]
fn identity_rejects_a_recycled_pid_with_the_same_name_but_a_different_start_time() {
    let listed_at = SystemTime::UNIX_EPOCH + Duration::from_secs(500);
    let relaunched_at = SystemTime::UNIX_EPOCH + Duration::from_secs(600);
    let subject = CaptureSubject::new("teams.exe", 100, listed_at);
    assert!(!identity_still_matches(
        &subject,
        "teams.exe",
        relaunched_at
    ));
}

#[test]
fn identity_rejects_a_different_process_name_at_the_same_pid() {
    let started_at = SystemTime::UNIX_EPOCH + Duration::from_secs(500);
    let subject = CaptureSubject::new("teams.exe", 100, started_at);
    assert!(!identity_still_matches(&subject, "malware.exe", started_at));
}
