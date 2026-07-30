//! Filters and deduplicates the render-device audio sessions a Windows
//! device collection reports into the capture subjects `list-sources`
//! shows.

use std::collections::{HashMap, HashSet};

use transcriber_core::CaptureSubject;

use crate::process_tree::{ProcessSnapshot, is_stop_listed, resolve_tree_root};

/// One audio session as reported by a render device's session enumerator,
/// translated from `wasapi::AudioSessionControl` before any pure logic here
/// sees it — kept free of the `wasapi` crate so this stays testable without
/// a device (see [`crate::sources`], this crate's one edge module).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RenderSession {
    pub(crate) process_id: u32,
    pub(crate) activity: SessionActivity,
}

/// Mirrors `wasapi::SessionState`'s three variants under this crate's own
/// name — see [`RenderSession`]'s doc for why it does not borrow
/// `wasapi::SessionState` directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionActivity {
    /// At least one stream in the session is running — `list-sources` shows
    /// exactly this state.
    Active,
    /// The session has streams, but none are currently running.
    Inactive,
    /// The session has no streams left.
    Expired,
}

/// A PID excluded from the source list outright: the caller's own process,
/// or a stop-listed process (by PID or by name) — the latter check matters
/// even here, before any tree walk, because a session literally *owned by*
/// a stop-listed process (e.g. system sounds hosted by `svchost.exe`) would
/// otherwise resolve to itself as the root and get offered as a capture
/// subject whose process tree is the shell or a service host, not one
/// application.
fn is_excluded(pid: u32, own_pid: u32, processes: &HashMap<u32, ProcessSnapshot>) -> bool {
    pid == own_pid || is_stop_listed(pid, processes)
}

/// Builds the deduplicated capture-subject list from every render session
/// and the process table observed at (approximately) the same moment.
///
/// Filters to [`SessionActivity::Active`]; excludes `own_pid` and any
/// stop-listed process, checked on both the raw session PID and the
/// resolved root, since either could coincide with one; and deduplicates on
/// the *resolved* tree root, not the raw session PID, so two sessions
/// belonging to two child processes of the same client produce one
/// subject, not two (the spec's dedup requirement). A session whose
/// resolved root has no matching process-table entry contributes nothing —
/// there is no name or start time to show the user.
pub(crate) fn active_capture_subjects(
    sessions: &[RenderSession],
    processes: &HashMap<u32, ProcessSnapshot>,
    own_pid: u32,
) -> Vec<CaptureSubject> {
    let mut seen_roots = HashSet::new();
    let mut subjects = Vec::new();
    for session in sessions {
        if session.activity != SessionActivity::Active {
            continue;
        }
        let root_pid = resolve_tree_root(session.process_id, processes);
        if is_excluded(session.process_id, own_pid, processes)
            || is_excluded(root_pid, own_pid, processes)
        {
            continue;
        }
        if !seen_roots.insert(root_pid) {
            continue;
        }
        if let Some(root_process) = processes.get(&root_pid) {
            subjects.push(CaptureSubject::new(
                root_process.name.clone(),
                root_pid,
                root_process.started_at,
            ));
        }
    }
    subjects
}

#[cfg(test)]
mod tests;
