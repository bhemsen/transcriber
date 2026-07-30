//! The source, manifest and dependency-graph audit for the capture crates.
//!
//! `docs/constitution.md` (Architecture principles) names the guarded crates
//! `audio`, `audio-win`, `asr` and `diarize`: they own no write path and no
//! HTTP client, and its Don'ts add "no `Serialize` for audio or embedding
//! types" — a Don't no negative trait bound can express. A symbol scan alone
//! only proves the *absence of named symbols* in source text, so this audit
//! adds a manifest and dependency-graph inspection on top — see
//! `docs/specs/spec-capture-foundation.md`, "Gates und Dokumente".
//!
//! **Path deviation, disclosed per the spec:** the constitution names this
//! test's path literally as `tests/no_write_paths.rs`. The workspace root is
//! a virtual manifest without a package, so a root `tests/` directory would
//! never compile and `cargo test --workspace` would never run it. This file
//! lives in `xtask` instead — already the home of the workspace gates.
//! `xtask` is deliberately never itself a guarded crate (see
//! [`audit::GUARDED_CRATES`]), so the audit can never scan its own source or
//! its own fixtures into a false positive.
//!
//! The pure detectors — source-text scan, dependency-name matching — live in
//! [`audit`] and are unit-tested there against in-memory fixtures; that is
//! the permanent proof that a deliberate violation fails the audit, not a
//! manual experiment made once and reverted. This file wires those same
//! detectors to the real crates on disk and to `cargo tree`, and never
//! writes a file.

mod audit;

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

type Fallible = Result<(), Box<dyn Error>>;

/// The workspace root, derived from `xtask`'s own manifest directory (cargo
/// sets `CARGO_MANIFEST_DIR` for every test binary) rather than the process's
/// current directory, which cargo does not guarantee for test binaries.
fn workspace_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(root) = manifest_dir.parent() else {
        panic!("xtask's Cargo.toml is expected to sit directly under the workspace root");
    };
    root.to_path_buf()
}

/// Collects every `.rs` file under `dir`, recursing into subdirectories.
fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_rust_files(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// Direct dependency names declared in `[dependencies]` of `manifest_path` —
/// the "manifest" layer of the audit, independent of the transitive graph
/// below. The actual parsing is [`audit::parse_direct_dependencies`], a pure
/// function unit-tested against fixtures; this wrapper only does the read.
fn direct_dependencies(manifest_path: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let text = fs::read_to_string(manifest_path)?;
    audit::parse_direct_dependencies(&text)
        .map_err(|reason| format!("{}: {reason}", manifest_path.display()).into())
}

/// The crate's full normal+build dependency graph, via `cargo tree` — the
/// transitive layer a source- or manifest-only scan cannot see, e.g. a
/// dependency that itself starts pulling in `serde`. `--offline --locked`
/// because nothing in this workspace's guarded crates may reach the
/// network, this audit included, and a stale lockfile should fail loudly
/// rather than have `cargo tree` silently resolve around it. `env!("CARGO")`
/// rather than a bare `"cargo"` runs the exact toolchain currently building,
/// not whatever `cargo` a PATH lookup would happen to find.
///
/// Fails loudly if the parsed graph does not even contain `package` itself
/// — cargo tree always lists the root package first, so its absence means
/// the parser or the command broke, not that the crate has no dependencies.
/// An empty-but-plausible result is exactly what a broken graph check would
/// also produce, so this is the difference between "clean" and "untested".
fn dependency_graph(root: &Path, package: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let output = Command::new(env!("CARGO"))
        .arg("tree")
        .arg("--offline")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .args(["-p", package, "-e", "normal,build", "--prefix", "none"])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("cargo tree -p {package} failed: {stderr}").into());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let names = audit::parse_dependency_graph_names(&stdout);
    if !names.iter().any(|name| name == package) {
        return Err(format!(
            "cargo tree -p {package} did not list the package itself — parser or command likely broken"
        )
        .into());
    }
    Ok(names)
}

/// Runs every layer of the audit — source text, manifest, dependency graph —
/// against one crate on disk and returns every violation found. Empty means
/// clean.
fn audit_crate(
    crate_dir: &Path,
    package: &str,
    root: &Path,
) -> Result<Vec<String>, Box<dyn Error>> {
    let mut findings = Vec::new();

    let mut files = Vec::new();
    collect_rust_files(&crate_dir.join("src"), &mut files)?;
    let build_script = crate_dir.join("build.rs");
    if build_script.exists() {
        files.push(build_script);
    }
    for file in &files {
        let source = fs::read_to_string(file)?;
        for finding in audit::scan_source_text(&source) {
            findings.push(format!("{}: {finding}", file.display()));
        }
    }

    let direct = direct_dependencies(&crate_dir.join("Cargo.toml"))?;
    for name in audit::blocked_dependencies_in(&direct, audit::BLOCKED_DEPENDENCY_CRATES) {
        findings.push(format!("manifest dependency: {name}"));
    }

    let graph = dependency_graph(root, package)?;
    for name in audit::blocked_dependencies_in(&graph, audit::BLOCKED_DEPENDENCY_CRATES) {
        findings.push(format!("dependency graph: {name}"));
    }

    Ok(findings)
}

/// The audit itself: every guarded crate that exists today must be clean.
/// Crates not built yet are skipped so this test grows with the phases
/// instead of breaking at each one.
#[test]
fn guarded_crates_that_exist_are_clean() -> Fallible {
    let root = workspace_root();
    for (dir, package) in audit::GUARDED_CRATES {
        let crate_dir = root.join("crates").join(dir);
        if !crate_dir.join("Cargo.toml").exists() {
            continue;
        }
        let findings = audit_crate(&crate_dir, package, &root)?;
        assert!(
            findings.is_empty(),
            "{dir} has audit violations: {findings:?}"
        );
    }
    Ok(())
}

/// Guards against the skip-missing-crates rule silently turning the whole
/// gate into a no-op — e.g. a path typo in [`audit::GUARDED_CRATES`] making
/// every entry look "not built yet" while the real crate sits right there.
/// A vacuous green gate is worse than none.
#[test]
fn at_least_one_guarded_crate_exists_so_the_audit_is_not_vacuous() {
    let root = workspace_root();
    let existing = audit::GUARDED_CRATES
        .iter()
        .filter(|(dir, _)| root.join("crates").join(dir).join("Cargo.toml").exists())
        .count();
    assert!(
        existing >= 1,
        "no guarded crate resolved to a real directory — check audit::GUARDED_CRATES for a path typo"
    );
}
