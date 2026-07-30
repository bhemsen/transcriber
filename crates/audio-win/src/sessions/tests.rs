//! Device-free logic tests for [`super::active_capture_subjects`] — split
//! into their own file so the shipped code in `sessions.rs` stays well
//! under the constitution's 400-line guideline (`docs/constitution.md`,
//! Architecture principles).

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use crate::process_tree::ProcessSnapshot;

use super::{RenderSession, SessionActivity, active_capture_subjects};

const OWN_PID: u32 = 1;

fn snapshot(name: &str, parent_pid: Option<u32>) -> ProcessSnapshot {
    ProcessSnapshot {
        name: name.to_string(),
        parent_pid,
        started_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1_000),
    }
}

fn session(process_id: u32, activity: SessionActivity) -> RenderSession {
    RenderSession {
        process_id,
        activity,
    }
}

#[test]
fn inactive_and_expired_sessions_do_not_appear() {
    let processes = HashMap::from([(100, snapshot("app.exe", None))]);
    let sessions = [
        session(100, SessionActivity::Inactive),
        session(100, SessionActivity::Expired),
    ];
    assert!(active_capture_subjects(&sessions, &processes, OWN_PID).is_empty());
}

#[test]
fn an_active_session_produces_a_subject() {
    let processes = HashMap::from([(100, snapshot("app.exe", None))]);
    let sessions = [session(100, SessionActivity::Active)];
    let subjects = active_capture_subjects(&sessions, &processes, OWN_PID);
    assert_eq!(subjects.len(), 1);
    assert_eq!(subjects[0].process_name(), "app.exe");
    assert_eq!(subjects[0].root_pid(), 100);
}

#[test]
fn the_callers_own_process_is_excluded_even_if_active() {
    let processes = HashMap::from([(OWN_PID, snapshot("harness.exe", None))]);
    let sessions = [session(OWN_PID, SessionActivity::Active)];
    assert!(active_capture_subjects(&sessions, &processes, OWN_PID).is_empty());
}

#[test]
fn stop_listed_pids_are_excluded_even_if_reported_active() {
    let processes = HashMap::from([(4, snapshot("system", None))]);
    let sessions = [session(4, SessionActivity::Active)];
    assert!(active_capture_subjects(&sessions, &processes, OWN_PID).is_empty());
}

#[test]
fn two_children_of_one_root_dedupe_into_a_single_subject_named_after_the_root() {
    let processes = HashMap::from([
        (100, snapshot("teams.exe", None)),
        (150, snapshot("teams_child_a.exe", Some(100))),
        (160, snapshot("teams_child_b.exe", Some(100))),
    ]);
    let sessions = [
        session(150, SessionActivity::Active),
        session(160, SessionActivity::Active),
    ];
    let subjects = active_capture_subjects(&sessions, &processes, OWN_PID);
    assert_eq!(subjects.len(), 1);
    assert_eq!(subjects[0].process_name(), "teams.exe");
    assert_eq!(subjects[0].root_pid(), 100);
}

#[test]
fn sessions_with_different_resolved_roots_produce_distinct_subjects() {
    let processes = HashMap::from([
        (100, snapshot("teams.exe", None)),
        (200, snapshot("chrome.exe", None)),
    ]);
    let sessions = [
        session(100, SessionActivity::Active),
        session(200, SessionActivity::Active),
    ];
    let mut roots: Vec<u32> = active_capture_subjects(&sessions, &processes, OWN_PID)
        .iter()
        .map(transcriber_core::CaptureSubject::root_pid)
        .collect();
    roots.sort_unstable();
    assert_eq!(roots, vec![100, 200]);
}

#[test]
fn a_session_whose_root_has_no_process_record_produces_no_subject() {
    let processes = HashMap::new();
    let sessions = [session(999, SessionActivity::Active)];
    assert!(active_capture_subjects(&sessions, &processes, OWN_PID).is_empty());
}
