//! Pure detectors behind the audit in `no_write_paths.rs`.
//!
//! Kept free of I/O on purpose: every function here takes an in-memory
//! string or list, so the unit tests below feed it fixtures directly rather
//! than touching disk — the permanent proof that a deliberate violation
//! fails the audit, not a one-off manual experiment that gets reverted.

/// (crate directory under `crates/`, Cargo package name) — the crates
/// `docs/constitution.md` (Architecture principles) names as owning no
/// write path and no HTTP client. `xtask` itself is deliberately never in
/// this list; see [`tests::guarded_crates_never_include_the_audit_tool_itself`]
/// below — the audit only ever reads the four crates named here, so it can
/// never scan its own source or its own fixtures into a false positive.
pub(crate) const GUARDED_CRATES: &[(&str, &str)] = &[
    ("audio", "transcriber-audio"),
    ("audio-win", "transcriber-audio-win"),
    ("asr", "transcriber-asr"),
    ("diarize", "transcriber-diarize"),
];

/// The constitution's symbol blocklist, plus `Serialize`/`serde` — a Don't
/// that no negative trait bound can express (spec, "Gates und Dokumente").
pub(crate) const BLOCKED_SOURCE_SYMBOLS: &[&str] = &[
    "fs::write",
    "fs::File::create",
    "OpenOptions::write",
    "reqwest",
    "ureq",
    "Serialize",
    "serde",
];

/// Crate names a guarded crate's dependency graph must never contain —
/// checked both as direct, manifest-declared dependencies and across the
/// full transitive `cargo tree` graph (see `no_write_paths.rs`).
pub(crate) const BLOCKED_DEPENDENCY_CRATES: &[&str] = &["serde", "reqwest", "ureq"];

/// Scans `source` for [`BLOCKED_SOURCE_SYMBOLS`] and returns one finding per
/// hit.
///
/// Whitespace is stripped before matching, so `std :: fs :: write` and a
/// `use std::fs::write as w;` rename both match on the qualified path the
/// `use` line still spells out — a plain exact-string search would miss
/// both (the two false-negative traps a naive scan invites). What this
/// cannot see is a *cross-crate* re-export that never repeats the path in
/// this crate's own source at all, e.g. a hand-rolled `impl Write` in an
/// unguarded helper crate; the dependency-graph check in
/// [`blocked_dependencies_in`] is the independent second line of defence for
/// that gap, for the three crate-level symbols (`reqwest`, `ureq`, `serde`).
///
/// `OpenOptions::write` gets an extra heuristic: idiomatic code almost never
/// spells that path as one qualified expression — it calls
/// `OpenOptions::new()` and then `.write(true)` as a separate builder step,
/// so the literal substring would never appear even in a real violation.
/// This also flags any file that mentions `OpenOptions` alongside a
/// `.write(` call.
pub(crate) fn scan_source_text(source: &str) -> Vec<String> {
    let normalized: String = source.chars().filter(|c| !c.is_whitespace()).collect();
    let mut findings: Vec<String> = BLOCKED_SOURCE_SYMBOLS
        .iter()
        .filter(|symbol| normalized.contains(**symbol))
        .map(|symbol| format!("blocked symbol `{symbol}`"))
        .collect();
    if normalized.contains("OpenOptions") && normalized.contains(".write(") {
        findings.push("OpenOptions builder pattern (`OpenOptions` + `.write(`)".to_string());
    }
    findings
}

/// Returns every name in `names` that also appears in `blocked` — the same
/// pure check run against a crate's direct manifest dependencies and against
/// its full `cargo tree` graph, so both layers share one tested rule.
pub(crate) fn blocked_dependencies_in(names: &[String], blocked: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter(|name| blocked.contains(&name.as_str()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        BLOCKED_DEPENDENCY_CRATES, GUARDED_CRATES, blocked_dependencies_in, scan_source_text,
    };

    #[test]
    fn rejects_fs_write() {
        let findings = scan_source_text(r#"std::fs::write("out.pcm", &data).ok();"#);
        assert!(!findings.is_empty());
    }

    #[test]
    fn rejects_fs_write_via_renamed_import() {
        let findings = scan_source_text("use std::fs::write as w;\nfn f() { w(\"p\", &[]).ok(); }");
        assert!(!findings.is_empty());
    }

    #[test]
    fn rejects_fs_write_with_unusual_spacing() {
        let findings = scan_source_text("std :: fs :: write(\"p\", &[]).ok();");
        assert!(!findings.is_empty());
    }

    #[test]
    fn rejects_fs_file_create() {
        let findings = scan_source_text("let _f = std::fs::File::create(\"p\");");
        assert!(!findings.is_empty());
    }

    #[test]
    fn rejects_open_options_write_builder_pattern() {
        let findings =
            scan_source_text("let mut opts = std::fs::OpenOptions::new();\nopts.write(true);");
        assert!(!findings.is_empty());
    }

    #[test]
    fn rejects_reqwest_usage() {
        let findings = scan_source_text("let _ = reqwest::blocking::get(\"http://x\");");
        assert!(!findings.is_empty());
    }

    #[test]
    fn rejects_ureq_usage() {
        let findings = scan_source_text("let _ = ureq::get(\"http://x\").call();");
        assert!(!findings.is_empty());
    }

    #[test]
    fn rejects_serialize_derive() {
        let findings = scan_source_text("#[derive(serde::Serialize)]\nstruct S { x: i32 }");
        assert!(!findings.is_empty());
    }

    #[test]
    fn accepts_clean_source() {
        let findings = scan_source_text(
            "use zeroize::Zeroizing;\n\npub struct RingBuffer { storage: Zeroizing<Vec<f32>> }",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn dependency_graph_rejects_serde_even_when_transitive() {
        let graph = vec![
            "transcriber-audio".to_string(),
            "thiserror".to_string(),
            "some-dependency".to_string(),
            "serde".to_string(),
        ];
        let findings = blocked_dependencies_in(&graph, BLOCKED_DEPENDENCY_CRATES);
        assert_eq!(findings, vec!["serde".to_string()]);
    }

    #[test]
    fn dependency_graph_accepts_clean_graph() {
        let graph = vec!["thiserror".to_string(), "zeroize".to_string()];
        let findings = blocked_dependencies_in(&graph, BLOCKED_DEPENDENCY_CRATES);
        assert!(findings.is_empty());
    }

    /// Structural guard for the self-scan trap: if `xtask` ever ended up in
    /// [`GUARDED_CRATES`], the audit would find its own blocked-symbol
    /// string literals and reject itself.
    #[test]
    fn guarded_crates_never_include_the_audit_tool_itself() {
        let leaked = GUARDED_CRATES
            .iter()
            .any(|(dir, package)| *dir == "xtask" || *package == "xtask");
        assert!(
            !leaked,
            "xtask must never audit itself — it contains the blocked symbols as string literals"
        );
    }
}
