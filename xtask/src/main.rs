//! Cross-platform tasks for tpt-text, run as `cargo xtask <task>`.
//!
//! - `ci`: formatting, lints, tests, `no_std` and WebAssembly builds, docs, and
//!   the wasm digest check (skipped when Node.js is not installed).
//! - `bench-check`: runs the benchmarks and fails if any case is more than 25%
//!   slower than `crates/tpt-text/benches/baseline.txt`. Pass `--update` to
//!   record the current timings as the baseline.
//! - `wasm-digests`: rebuilds the determinism module and rewrites its recorded
//!   digests from the WebAssembly build.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

/// Allowed slowdown before `bench-check` fails.
const BENCH_THRESHOLD: f64 = 1.25;

/// The WebAssembly module used for the determinism check.
const WASM_MODULE: &str = "target/wasm32-unknown-unknown/release/tpt_text_wasm_check.wasm";
/// Digests recorded from the WebAssembly build.
const RECORDED_DIGESTS: &str = "crates/tpt-text-wasm-check/golden/digests.json";

fn main() {
    let task = env::args().nth(1).unwrap_or_default();
    let rest: Vec<String> = env::args().skip(2).collect();
    let result = match task.as_str() {
        "ci" => ci(),
        "bench-check" => bench_check(rest.iter().any(|a| a == "--update")),
        "wasm-digests" => wasm_digests(),
        _ => {
            eprintln!("usage: cargo xtask <ci | bench-check [--update] | wasm-digests>");
            process::exit(2);
        }
    };
    if let Err(message) = result {
        eprintln!("xtask: {message}");
        process::exit(1);
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Runs `program` with `args` from the repository root and fails on a non-zero exit.
fn run(program: &str, args: &[&str]) -> Result<(), String> {
    println!("==> {program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .current_dir(root())
        .status()
        .map_err(|e| format!("starting {program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{program} {}` failed", args.join(" ")))
    }
}

/// True if `program` can be started and exits successfully.
fn available(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .current_dir(root())
        .output()
        .is_ok_and(|o| o.status.success())
}

fn build_wasm_module() -> Result<(), String> {
    run(
        "cargo",
        &[
            "build",
            "-p",
            "tpt-text-wasm-check",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ],
    )
}

fn ci() -> Result<(), String> {
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
    run("cargo", &["test", "-p", "tpt-text", "--features", "serde"])?;
    run("cargo", &["test", "-p", "tpt-text", "--features", "fontdb"])?;
    run(
        "cargo",
        &["build", "-p", "tpt-text", "--example", "layout_demo"],
    )?;
    run(
        "cargo",
        &[
            "build",
            "-p",
            "tpt-text",
            "--no-default-features",
            "--target",
            "thumbv7em-none-eabihf",
        ],
    )?;
    run(
        "cargo",
        &[
            "build",
            "-p",
            "tpt-text",
            "--no-default-features",
            "--target",
            "wasm32-unknown-unknown",
        ],
    )?;
    build_wasm_module()?;
    // Documentation builds the way docs.rs builds it: `docsrs` cfg, warnings denied.
    env::set_var("RUSTDOCFLAGS", "--cfg docsrs -D warnings");
    let docs = run("cargo", &["doc", "-p", "tpt-text", "--no-deps"]);
    env::remove_var("RUSTDOCFLAGS");
    docs?;
    check_wasm_digests()?;
    println!("All checks passed.");
    Ok(())
}

/// Rebuilds the determinism module and compares its digests with the recorded
/// file. Skipped with a notice if Node.js is not installed.
fn check_wasm_digests() -> Result<(), String> {
    if !available("node", &["--version"]) {
        println!("==> skipping wasm digest check: Node.js not found");
        return Ok(());
    }
    let fresh = "target/wasm-digests-check.json";
    run("node", &["scripts/wasm-digest.mjs", WASM_MODULE, fresh])?;
    let expected = fs::read_to_string(root().join(RECORDED_DIGESTS)).map_err(|e| e.to_string())?;
    let actual = fs::read_to_string(root().join(fresh)).map_err(|e| e.to_string())?;
    fs::remove_file(root().join(fresh)).ok();
    if expected != actual {
        return Err(format!(
            "wasm digests differ from {RECORDED_DIGESTS}; run `cargo xtask wasm-digests` after checking the change"
        ));
    }
    Ok(())
}

fn wasm_digests() -> Result<(), String> {
    build_wasm_module()?;
    run(
        "node",
        &["scripts/wasm-digest.mjs", WASM_MODULE, RECORDED_DIGESTS],
    )
}

/// Runs the benchmarks, then compares or records the timings.
fn bench_check(update: bool) -> Result<(), String> {
    println!("==> cargo bench -p tpt-text");
    let output = Command::new("cargo")
        .args(["bench", "-p", "tpt-text"])
        .current_dir(root())
        .output()
        .map_err(|e| format!("starting cargo bench: {e}"))?;
    if !output.status.success() {
        return Err("cargo bench failed".into());
    }
    let timings = parse_timings(&String::from_utf8_lossy(&output.stdout));
    if timings.is_empty() {
        return Err("no timings found in benchmark output".into());
    }
    let baseline_path = root().join("crates/tpt-text/benches/baseline.txt");

    if update {
        let body: String = timings
            .iter()
            .map(|(name, ms)| format!("{name}\t{ms:.3}\n"))
            .collect();
        fs::write(&baseline_path, body).map_err(|e| e.to_string())?;
        println!(
            "recorded {} timings in {}",
            timings.len(),
            baseline_path.display()
        );
        return Ok(());
    }

    let baseline = fs::read_to_string(&baseline_path).map_err(|e| {
        format!(
            "reading {}: {e}; record one with --update",
            baseline_path.display()
        )
    })?;
    let recorded = parse_baseline(&baseline);
    let mut failures = Vec::new();
    for (name, ms) in &timings {
        match recorded.get(name) {
            Some(base) if *ms > base * BENCH_THRESHOLD => {
                failures.push(format!("{name}: {ms:.3} ms vs baseline {base:.3} ms"));
            }
            Some(base) => println!("{name}: {ms:.3} ms (baseline {base:.3} ms)"),
            None => println!("{name}: {ms:.3} ms (no baseline)"),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "slower than {BENCH_THRESHOLD}x the baseline:\n  {}",
            failures.join("\n  ")
        ))
    }
}

/// Reads `name ... <value> ms/iter` lines written by the benchmark harness.
fn parse_timings(output: &str) -> BTreeMap<String, f64> {
    output
        .lines()
        .filter_map(|line| {
            let (before, _) = line.split_once(" ms/iter")?;
            let value: f64 = before.split_whitespace().next_back()?.parse().ok()?;
            let words: Vec<&str> = before.split_whitespace().collect();
            let name = words.get(..words.len().saturating_sub(1))?.join(" ");
            Some((name, value))
        })
        .collect()
}

/// Reads the tab-separated baseline file.
fn parse_baseline(text: &str) -> BTreeMap<String, f64> {
    text.lines()
        .filter_map(|line| {
            let (name, ms) = line.split_once('\t')?;
            Some((name.to_string(), ms.trim().parse().ok()?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_benchmark_lines() {
        let out = "latin 10k chars, 480px      1.234 ms/iter (20 iters)\nnoise\n";
        let timings = parse_timings(out);
        assert_eq!(timings.get("latin 10k chars, 480px"), Some(&1.234));
        assert_eq!(timings.len(), 1);
    }

    #[test]
    fn reads_the_baseline_format() {
        let parsed = parse_baseline("latin\t1.500\narabic\t2.250\n");
        assert_eq!(parsed.get("arabic"), Some(&2.25));
    }
}
