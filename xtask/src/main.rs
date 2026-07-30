//! Task runner for the workspace.
//!
//! The project exposes exactly two entry points to the loopkit skills, both
//! defined in `docs/workflow.md`:
//!
//! - `cargo xtask bootstrap` — make a fresh checkout or worktree runnable.
//! - `cargo xtask verify` — the per-iteration gate; one non-interactive command.
//!
//! Frontend steps activate automatically once `frontend/package.json` exists
//! (phase 5), so the command names never change.

use std::error::Error;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::time::Instant;

type Fallible = Result<(), Box<dyn Error>>;

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let result = match task.as_str() {
        "bootstrap" => bootstrap(),
        "verify" => verify(),
        "build" => build(),
        other => {
            eprintln!("unknown task {other:?}; expected bootstrap | verify | build");
            return ExitCode::FAILURE;
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("\n{task} failed: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Installs everything a fresh worktree needs to run Verify.
fn bootstrap() -> Fallible {
    run("cargo", &["fetch", "--locked"])?;
    if has_frontend() {
        run("npm", &["ci", "--prefix", "frontend"])?;
    }
    println!("\nbootstrap complete");
    Ok(())
}

/// The per-iteration gate. Stops at the first failing step.
fn verify() -> Fallible {
    let started = Instant::now();
    run("cargo", &["fmt", "--all", "--", "--check"])?;
    run(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run("cargo", &["test", "--workspace"])?;
    run("cargo", &["deny", "check"])?;
    if has_frontend() {
        run("npm", &["--prefix", "frontend", "run", "verify"])?;
    }
    println!("\nverify green in {:.1}s", started.elapsed().as_secs_f64());
    Ok(())
}

/// The full check for app-affecting changes.
fn build() -> Fallible {
    run("cargo", &["build", "--workspace", "--all-targets"])?;
    if has_frontend() {
        run("npm", &["--prefix", "frontend", "run", "build"])?;
    }
    Ok(())
}

fn has_frontend() -> bool {
    Path::new("frontend/package.json").exists()
}

/// Runs a command, streaming its output, and fails on a non-zero exit.
fn run(program: &str, args: &[&str]) -> Fallible {
    println!("\n> {program} {}", args.join(" "));
    let status = Command::new(program).args(args).status().map_err(|error| {
        format!("could not start {program:?} ({error}) — see docs/workflow.md prerequisites")
    })?;
    if status.success() {
        return Ok(());
    }
    Err(format!("{program} {} exited with {status}", args.join(" ")).into())
}
