//! Process-tree root resolution and the identity re-check.
//!
//! Both are pure functions over a caller-supplied process table rather than
//! calls into `sysinfo` themselves — the shape the spec's Verification list
//! requires so they run on a CI runner with no audio device
//! (`docs/specs/spec-capture-foundation.md`).

use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

use transcriber_core::CaptureSubject;

/// Process names [`resolve_tree_root`]'s ancestor walk never climbs past:
/// shell and service hosts that each own countless unrelated child
/// processes. Climbing into one of these would resolve to a shared root and
/// capture applications the user never selected — the spec's named risk for
/// an unbounded walk.
pub(crate) const STOP_PROCESS_NAMES: &[&str] =
    &["explorer.exe", "services.exe", "svchost.exe", "wininit.exe"];

/// PIDs the walk never climbs into: the Idle process and the System
/// process, neither of which is ever a real capture target.
pub(crate) const STOP_PIDS: &[u32] = &[0, 4];

/// A process observed at one point in time, translated from
/// `sysinfo::Process` before any pure logic in this crate sees it — the
/// platform-neutral shape that keeps [`resolve_tree_root`] and
/// [`crate::sessions::active_capture_subjects`] testable without a real
/// process table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessSnapshot {
    pub(crate) name: String,
    pub(crate) parent_pid: Option<u32>,
    pub(crate) started_at: SystemTime,
}

/// Whether `pid` is a stop-listed boundary the walk must not climb into —
/// by PID first (correct even with no process-table entry for it), then by
/// name for whichever of the four stop-listed executables `processes` has a
/// record of at `pid`.
fn is_stop_listed(pid: u32, processes: &HashMap<u32, ProcessSnapshot>) -> bool {
    if STOP_PIDS.contains(&pid) {
        return true;
    }
    processes.get(&pid).is_some_and(|process| {
        STOP_PROCESS_NAMES
            .iter()
            .any(|stop| stop.eq_ignore_ascii_case(&process.name))
    })
}

/// Resolves `pid` to its process-tree root, climbing parent links in
/// `processes` — but only up to the stop list.
///
/// A call client's render session is usually owned by a child process, so
/// the walk must climb at least one level (the spec's reference-example
/// warning: the parent PID is the real capture target). But an unbounded
/// climb would reach `explorer.exe` and capture every application on the
/// desktop, breaking source isolation — the spec's other named risk for the
/// same walk.
///
/// Stops, returning the last resolved PID, at the first of: `pid` itself
/// already stop-listed; a parent PID the walk already visited (a cycle in
/// the process table, never trusted past the point it repeats); a parent
/// PID that is stop-listed by PID or by name; or a parent PID `processes`
/// has no record for (the parent already exited — nothing to verify past
/// it, so the walk does not climb into the unknown).
pub(crate) fn resolve_tree_root(pid: u32, processes: &HashMap<u32, ProcessSnapshot>) -> u32 {
    if is_stop_listed(pid, processes) {
        return pid;
    }
    let mut current = pid;
    let mut visited = HashSet::from([pid]);
    loop {
        let Some(process) = processes.get(&current) else {
            return current;
        };
        let Some(parent_pid) = process.parent_pid else {
            return current;
        };
        if !visited.insert(parent_pid) || is_stop_listed(parent_pid, processes) {
            return current;
        }
        if !processes.contains_key(&parent_pid) {
            return current;
        }
        current = parent_pid;
    }
}

/// Whether the application named in `subject` is still the same process it
/// was when `subject` was listed — checked by *name and start time*, not
/// the PID alone, because Windows recycles PIDs.
///
/// `current_name` and `current_started_at` are what a fresh process-table
/// lookup at `subject.root_pid()` reports right now, at capture time. A
/// mismatch means that PID now names a different process than the one the
/// user consented to — capturing it anyway would be a consent breach, not
/// just a bug (the spec's own wording for exactly this check). Exposed as
/// part of this crate's public surface for `open_remote`'s future
/// implementation (issue #12) to call between listing and opening.
#[must_use]
pub fn identity_still_matches(
    subject: &CaptureSubject,
    current_name: &str,
    current_started_at: SystemTime,
) -> bool {
    subject.process_name() == current_name && subject.started_at() == current_started_at
}

#[cfg(test)]
mod tests;
