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
///
/// `File::create` stands in for the constitution's literal `fs::File::create`
/// too: it is a substring of that longer path, so it still matches a fully
/// qualified call, and it *additionally* matches the idiomatic
/// `use std::fs::File; File::create(...)` form real code actually uses (and
/// `File::create_new`, since `"File::create_new".contains("File::create")`).
pub(crate) const BLOCKED_SOURCE_SYMBOLS: &[&str] = &[
    "fs::write",
    "File::create",
    "OpenOptions::write",
    "reqwest",
    "ureq",
    "Serialize",
    "serde",
];

/// Crate name *prefixes* a guarded crate's dependency graph must never
/// contain — checked both as direct, manifest-declared dependencies and
/// across the full transitive `cargo tree` graph (see `no_write_paths.rs`).
/// Prefix, not exact match, so a split-out sibling such as `serde_core` or
/// `serde_derive` cannot arrive under the family name while dodging an
/// exact-string check.
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
/// `OpenOptions::write` and `File::options().write(...)` get an extra
/// builder-pattern heuristic each: idiomatic code almost never spells either
/// path as one qualified expression — it calls `OpenOptions::new()` /
/// `File::options()` and then `.write(true)` as a separate step, so the
/// literal substring would never appear even in a real violation. Both
/// heuristics fire on the constructor call plus a later `.write(` anywhere
/// in the same file.
pub(crate) fn scan_source_text(source: &str) -> Vec<String> {
    let normalized: String = source.chars().filter(|c| !c.is_whitespace()).collect();
    let mut findings: Vec<String> = BLOCKED_SOURCE_SYMBOLS
        .iter()
        .filter(|symbol| normalized.contains(**symbol))
        .map(|symbol| format!("blocked symbol `{symbol}`"))
        .collect();
    let has_write_call = normalized.contains(".write(");
    if normalized.contains("OpenOptions") && has_write_call {
        findings.push("OpenOptions builder pattern (`OpenOptions` + `.write(`)".to_string());
    }
    if normalized.contains("File::options()") && has_write_call {
        findings
            .push("File::options() builder pattern (`File::options()` + `.write(`)".to_string());
    }
    findings
}

/// Returns every name in `names` that starts with one of the `blocked`
/// prefixes — the same pure check run against a crate's direct manifest
/// dependencies and against its full `cargo tree` graph, so both layers
/// share one tested rule.
pub(crate) fn blocked_dependencies_in(names: &[String], blocked: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter(|name| blocked.iter().any(|prefix| name.starts_with(prefix)))
        .cloned()
        .collect()
}

/// Parses the `[dependencies]` table of a `Cargo.toml`'s text into direct
/// dependency names.
///
/// Handles the flat `name = "version"` / `name = { ... }` form every
/// guarded crate's `Cargo.toml` uses today, one dependency per line. Two
/// disclosed scope limits, not silent gaps: a `[dependencies.foo]` sub-table
/// would not be recognised, and neither would a platform-specific table
/// such as `[target.'cfg(windows)'.dependencies]` — the crate that will
/// need the latter, `audio-win`, does not exist yet. Returns an error
/// instead of an empty, falsely "clean" result if the manifest never
/// contains a plain `[dependencies]` header at all, since that is the
/// signal this parser understood nothing rather than that there is nothing
/// to find.
pub(crate) fn parse_direct_dependencies(manifest_text: &str) -> Result<Vec<String>, String> {
    let mut in_dependencies = false;
    let mut saw_dependencies_header = false;
    let mut names = Vec::new();
    for raw_line in manifest_text.lines() {
        let line = raw_line.trim();
        if let Some(header) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            in_dependencies = header == "dependencies";
            saw_dependencies_header |= in_dependencies;
            continue;
        }
        if !in_dependencies || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((name, _)) = line.split_once('=') {
            names.push(name.trim().to_string());
        }
    }
    if !saw_dependencies_header {
        return Err("no [dependencies] table found — parser may be stale".to_string());
    }
    Ok(names)
}

/// Parses `cargo tree --prefix none`'s stdout into one crate name per line —
/// the first whitespace-separated token, which is the name regardless of
/// version suffix, `(proc-macro)`/`(*)` annotation, or `[build-dependencies]`
/// section labels the real output also contains (a section label has no
/// version after it and never matches a blocked prefix, so it is harmless
/// noise rather than a false positive).
pub(crate) fn parse_dependency_graph_names(cargo_tree_output: &str) -> Vec<String> {
    cargo_tree_output
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        BLOCKED_DEPENDENCY_CRATES, GUARDED_CRATES, blocked_dependencies_in,
        parse_dependency_graph_names, parse_direct_dependencies, scan_source_text,
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
    fn rejects_fs_file_create_fully_qualified() {
        let findings = scan_source_text("let _f = std::fs::File::create(\"p\");");
        assert!(!findings.is_empty());
    }

    /// The idiomatic form real code actually uses — an unqualified `use`
    /// followed by a bare call — which a naive check for the fully
    /// qualified `fs::File::create` path would miss entirely.
    #[test]
    fn rejects_file_create_via_unqualified_import() {
        let findings = scan_source_text("use std::fs::File;\nlet _f = File::create(\"dump.pcm\");");
        assert!(!findings.is_empty());
    }

    #[test]
    fn rejects_file_create_new() {
        let findings = scan_source_text("use std::fs::File;\nlet _f = File::create_new(\"p\");");
        assert!(!findings.is_empty());
    }

    #[test]
    fn rejects_open_options_write_builder_pattern() {
        let findings =
            scan_source_text("let mut opts = std::fs::OpenOptions::new();\nopts.write(true);");
        assert!(!findings.is_empty());
    }

    /// `File::options()` is `OpenOptions::new()` under a stable-since-1.75
    /// alias; the heuristic written for `OpenOptions` must catch this name
    /// too or the alias becomes a free pass.
    #[test]
    fn rejects_file_options_write_builder_pattern() {
        let findings =
            scan_source_text("let _f = std::fs::File::options().write(true).open(\"p\").ok();");
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

    /// `serde` split into `serde_core` after 1.0.220; a family member
    /// arriving without the bare `serde` crate itself must still be caught.
    #[test]
    fn dependency_graph_rejects_serde_family_members() {
        let graph = vec!["serde_core".to_string(), "serde_derive".to_string()];
        let findings = blocked_dependencies_in(&graph, BLOCKED_DEPENDENCY_CRATES);
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn dependency_graph_accepts_clean_graph() {
        let graph = vec!["thiserror".to_string(), "zeroize".to_string()];
        let findings = blocked_dependencies_in(&graph, BLOCKED_DEPENDENCY_CRATES);
        assert!(findings.is_empty());
    }

    #[test]
    fn manifest_parser_reads_flat_dependency_lines() {
        let manifest = "[package]\nname = \"x\"\n\n[dependencies]\nthiserror = \"2\"\nserde = { version = \"1\" }\n";
        let Ok(names) = parse_direct_dependencies(manifest) else {
            panic!("a manifest with a [dependencies] header must parse");
        };
        assert_eq!(names, vec!["thiserror".to_string(), "serde".to_string()]);
    }

    /// A manifest with no `[dependencies]` header at all (e.g. only
    /// `[dependencies.foo]` sub-tables, or a `[target...dependencies]`
    /// table) must fail loudly rather than silently report "no
    /// dependencies" — the vacuous-gate trap applied to this parser.
    #[test]
    fn manifest_parser_errors_without_a_dependencies_header() {
        let manifest = "[package]\nname = \"x\"\n\n[dependencies.thiserror]\nversion = \"2\"\n";
        assert!(parse_direct_dependencies(manifest).is_err());
    }

    #[test]
    fn dependency_graph_parser_reads_the_first_token_per_line() {
        let output = "transcriber-audio v0.1.0 (C:\\x)\nthiserror v2.0.19\nproc-macro2 v1.0.107 (*)\n[build-dependencies]\nzeroize v1.9.0\n";
        let names = parse_dependency_graph_names(output);
        assert_eq!(
            names,
            vec![
                "transcriber-audio".to_string(),
                "thiserror".to_string(),
                "proc-macro2".to_string(),
                "[build-dependencies]".to_string(),
                "zeroize".to_string(),
            ]
        );
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
