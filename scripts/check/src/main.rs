//! Runs every quality gate of this repository, in order, stopping at the first failure.
//!
//! Usage: `cargo run --locked --manifest-path scripts/check/Cargo.toml` (the same command runs locally, in the
//! factory and in CI); append gate names after `--` to run only those, in the standard order.
//!
//! Gates: rustfmt, clippy (`-D warnings`, every target and feature), semgrep (Semgrep CE with the factory's own
//! rules in .semgrep/), tests (cargo-llvm-cov) with line coverage >= 80 % of the library plus the doc tests,
//! documentation manifest, secrets (gitleaks), known vulnerabilities (cargo-audit over Cargo.lock) and
//! OSV-Scanner over Cargo.lock (high severity). No shell: it behaves the same on Windows and Linux.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::Value;

const MIN_COVERAGE: f64 = 80.0;
const DOCS_MANIFEST: &str = "docs-manifest.json";
const ALLOW_MISSING_GITLEAKS: &str = "FABRICA_ALLOW_MISSING_GITLEAKS";
const ALLOW_OFFLINE_AUDIT: &str = "FABRICA_ALLOW_OFFLINE_AUDIT";
/// `FABRICA_CI=1` also runs the tests behind `#![cfg(feature = "conformance")]` (external tools or downloads);
/// only the CI conformance job sets it, so local gates and coverage skip them.
const CI_TIER: &str = "FABRICA_CI";
const CONFORMANCE_FEATURE: &str = "conformance";
/// The check tool's own manifest: it is formatted and linted with the library.
const CHECK_MANIFEST: &str = "scripts/check/Cargo.toml";
/// Pinned; docker/rust-gates.Dockerfile in the factory and the central CI install the same versions.
const LLVM_COV_INSTALL: &str =
    "cargo install --locked cargo-llvm-cov@0.9.1 (and rustup component add llvm-tools-preview)";
const AUDIT_INSTALL: &str = "cargo install --locked cargo-audit@0.22.2";
/// The factory's own Semgrep rules; Semgrep-maintained rules are never used (license).
const SEMGREP_RULES: &str = ".semgrep/factory-rules.yml";
const OSV_LOCKFILE: &str = "Cargo.lock";
const HIGH_CVSS: f64 = 7.0;
/// osv-scanner's exit code when the lockfile lists no package.
const OSV_NO_PACKAGES: i32 = 128;

type GateResult = Result<(), String>;
type Gate = (&'static str, fn() -> GateResult);

const GATES: [Gate; 8] = [
    ("format", format_gate),
    ("lint", lint_gate),
    ("semgrep", semgrep_gate),
    ("tests", tests_gate),
    ("docs", docs_gate),
    ("secrets", secrets_gate),
    ("audit", audit_gate),
    ("osv", osv_gate),
];

fn main() -> ExitCode {
    if !Path::new("Cargo.toml").is_file() || !Path::new(CHECK_MANIFEST).is_file() {
        eprintln!("check: run from the repository root (Cargo.toml not found)");
        return ExitCode::from(2);
    }
    let only: Vec<String> = env::args().skip(1).collect();
    if let Some(unknown) = only
        .iter()
        .find(|name| !GATES.iter().any(|(gate, _)| gate == name))
    {
        eprintln!("check: unknown gate {unknown:?}");
        return ExitCode::from(2);
    }
    for (name, gate) in GATES {
        if !only.is_empty() && !only.iter().any(|o| o == name) {
            continue;
        }
        println!("==> {name}");
        if let Err(err) = gate() {
            eprintln!("{name}: {err}");
            eprintln!("==> {name}: FAILED");
            return ExitCode::from(1);
        }
        println!("==> {name}: ok");
    }
    println!("==> all gates passed");
    ExitCode::SUCCESS
}

fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn ci_tier() -> bool {
    env::var(CI_TIER).is_ok_and(|v| v == "1")
}

fn flag(name: &str) -> bool {
    env::var(name).is_ok_and(|v| v == "1")
}

/// Runs a tool from the repository root, streaming its output.
fn run(program: &str, args: &[&str]) -> GateResult {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|err| format!("cannot run {program}: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{program} {}` failed ({status})", args.join(" ")))
    }
}

/// Whether `program args` runs and exits 0 (used to probe for an installed tool).
fn probe(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .output()
        .is_ok_and(|out| out.status.success())
}

/// The program's path when it is on PATH (with the Windows executable suffixes).
fn which(program: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    let suffixes: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat", ""]
    } else {
        &[""]
    };
    env::split_paths(&path).find_map(|dir| {
        suffixes
            .iter()
            .map(|suffix| dir.join(format!("{program}{suffix}")))
            .find(|candidate| candidate.is_file())
    })
}

fn format_gate() -> GateResult {
    let cargo = cargo();
    run(&cargo, &["fmt", "--all", "--check"]).map_err(|e| format!("{e}: run `cargo fmt --all`"))?;
    run(
        &cargo,
        &["fmt", "--manifest-path", CHECK_MANIFEST, "--check"],
    )?;
    println!("rustfmt: every file is formatted");
    Ok(())
}

/// Clippy on every target with every feature, so the CI-only conformance tier compiles here and not first in CI.
fn lint_gate() -> GateResult {
    let cargo = cargo();
    run(
        &cargo,
        &[
            "clippy",
            "--locked",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run(
        &cargo,
        &[
            "clippy",
            "--locked",
            "--manifest-path",
            CHECK_MANIFEST,
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )
}

fn feature_args() -> Vec<&'static str> {
    if ci_tier() {
        vec!["--features", CONFORMANCE_FEATURE]
    } else {
        Vec::new()
    }
}

fn tests_gate() -> GateResult {
    let cargo = cargo();
    if !probe(&cargo, &["llvm-cov", "--version"]) {
        return Err(format!(
            "cargo-llvm-cov is not installed: {LLVM_COV_INSTALL}"
        ));
    }
    if ci_tier() {
        println!("tests: CI tier, the conformance feature is enabled");
    }
    let dir = env::temp_dir().join(format!("fabrica-cover-{}", std::process::id()));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let report = dir.join("coverage.json");
    let report_arg = report.to_string_lossy().into_owned();
    let mut args = vec![
        "llvm-cov",
        "--locked",
        "--json",
        "--summary-only",
        "--output-path",
        report_arg.as_str(),
    ];
    args.extend(feature_args());
    let measured = run(&cargo, &args).and_then(|()| {
        let text = fs::read_to_string(&report).map_err(|e| format!("no coverage report: {e}"))?;
        line_coverage(&text)
    });
    let _ = fs::remove_dir_all(&dir);
    let total = measured?;
    println!("Total coverage: {total:.2}%");
    if total < MIN_COVERAGE {
        return Err(format!(
            "total line coverage {total:.1}% is below {MIN_COVERAGE:.0}%"
        ));
    }
    println!("tests: line coverage {total:.1}% (minimum {MIN_COVERAGE:.0}%)");
    let mut doc = vec!["test", "--locked", "--doc"];
    doc.extend(feature_args());
    run(&cargo, &doc)
}

/// The line coverage percentage of a `cargo llvm-cov --json --summary-only` report.
fn line_coverage(report: &str) -> Result<f64, String> {
    let value: Value =
        serde_json::from_str(report).map_err(|e| format!("unreadable coverage report: {e}"))?;
    value["data"][0]["totals"]["lines"]["percent"]
        .as_f64()
        .ok_or_else(|| "no line total in the coverage report".to_string())
}

fn read_text(root: &Path, rel: &str) -> Option<String> {
    let path = root.join(rel);
    if !path.is_file() {
        return None;
    }
    fs::read_to_string(path).ok()
}

fn count_lines(text: &str) -> usize {
    let text = text.replace("\r\n", "\n");
    if text.is_empty() {
        0
    } else {
        text.trim_end_matches('\n').split('\n').count()
    }
}

/// Matches one path segment against a glob segment with `*`, `?` and `[...]` (ranges such as `0-9`).
fn segment_matches(pattern: &[char], name: &[char]) -> bool {
    match pattern.first() {
        None => name.is_empty(),
        Some('*') => (0..=name.len()).any(|skip| segment_matches(&pattern[1..], &name[skip..])),
        Some('?') => !name.is_empty() && segment_matches(&pattern[1..], &name[1..]),
        Some('[') => {
            let Some(close) = pattern.iter().position(|&c| c == ']') else {
                return name.first() == Some(&'[') && segment_matches(&pattern[1..], &name[1..]);
            };
            let Some(&c) = name.first() else { return false };
            let class = &pattern[1..close];
            let mut hit = false;
            let mut i = 0;
            while i < class.len() {
                if i + 2 < class.len() && class[i + 1] == '-' {
                    hit |= class[i] <= c && c <= class[i + 2];
                    i += 3;
                } else {
                    hit |= class[i] == c;
                    i += 1;
                }
            }
            hit && segment_matches(&pattern[close + 1..], &name[1..])
        }
        Some(&p) => name.first() == Some(&p) && segment_matches(&pattern[1..], &name[1..]),
    }
}

/// Whether some regular file under `dir` matches the remaining glob `segments`.
fn glob_has_file(dir: &Path, segments: &[&str]) -> bool {
    let Some((first, rest)) = segments.split_first() else {
        return false;
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    let pattern: Vec<char> = first.chars().collect();
    entries.flatten().any(|entry| {
        let name: Vec<char> = entry.file_name().to_string_lossy().chars().collect();
        if !segment_matches(&pattern, &name) {
            return false;
        }
        let path = entry.path();
        if rest.is_empty() {
            path.is_file()
        } else {
            path.is_dir() && glob_has_file(&path, rest)
        }
    })
}

/// Every violation of docs-manifest.json; an empty list means the gate passes.
fn docs_errors(root: &Path) -> Result<Vec<String>, String> {
    let raw = read_text(root, DOCS_MANIFEST).ok_or_else(|| format!("{DOCS_MANIFEST} not found"))?;
    let manifest: Value =
        serde_json::from_str(&raw).map_err(|e| format!("{DOCS_MANIFEST}: {e}"))?;
    let strings = |key: &str| -> Vec<String> {
        manifest[key]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut errors = Vec::new();
    for rel in strings("required") {
        match read_text(root, &rel) {
            None => errors.push(format!("missing required file: {rel}")),
            Some(text) if text.trim().is_empty() => {
                errors.push(format!("required file is empty: {rel}"))
            }
            Some(_) => {}
        }
    }
    for pattern in strings("required_globs") {
        let segments: Vec<&str> = pattern.split('/').collect();
        if !glob_has_file(root, &segments) {
            errors.push(format!("no file matches required pattern: {pattern}"));
        }
    }
    if let Some(limits) = manifest["max_lines"].as_object() {
        for (rel, limit) in limits {
            let limit = usize::try_from(limit.as_u64().unwrap_or(0)).unwrap_or(usize::MAX);
            if let Some(text) = read_text(root, rel) {
                let count = count_lines(&text);
                if count > limit {
                    errors.push(format!("{rel} has {count} lines (max {limit})"));
                }
            }
        }
    }
    if let Some(needles) = manifest["must_reference"].as_object() {
        for (rel, needle) in needles {
            let needle = needle.as_str().unwrap_or_default();
            if let Some(text) = read_text(root, rel)
                && !text.contains(needle)
            {
                errors.push(format!("{rel} must reference {needle}"));
            }
        }
    }
    Ok(errors)
}

fn docs_gate() -> GateResult {
    let errors = docs_errors(Path::new("."))?;
    for err in &errors {
        eprintln!("docs: {err}");
    }
    if !errors.is_empty() {
        return Err(format!("{} documentation problem(s)", errors.len()));
    }
    println!("docs: every file in docs-manifest.json is present");
    Ok(())
}

fn secrets_gate() -> GateResult {
    let Some(exe) = which("gitleaks") else {
        if flag(ALLOW_MISSING_GITLEAKS) {
            println!("gitleaks: SKIPPED (not installed)");
            return Ok(());
        }
        return Err(format!(
            "FAIL - gitleaks is not installed. Install it (https://github.com/gitleaks/gitleaks#installing), \
             e.g. `winget install Gitleaks.Gitleaks`, `brew install gitleaks` or a release binary, \
             or set {ALLOW_MISSING_GITLEAKS}=1 to skip this gate locally."
        ));
    };
    run(
        &exe.to_string_lossy(),
        &["dir", ".", "--redact", "--verbose", "--no-banner"],
    )
}

/// cargo-audit checks Cargo.lock against the RustSec advisory database (it downloads the database).
fn audit_gate() -> GateResult {
    let cargo = cargo();
    if !probe(&cargo, &["audit", "--version"]) {
        if flag(ALLOW_OFFLINE_AUDIT) {
            println!("cargo-audit: SKIPPED (not installed)");
            return Ok(());
        }
        return Err(format!(
            "cargo-audit is not installed: {AUDIT_INSTALL}, or set {ALLOW_OFFLINE_AUDIT}=1 to skip this gate locally"
        ));
    }
    // Vulnerabilities fail; informational warnings (unmaintained, yanked) are printed but do not fail.
    run(&cargo, &["audit", "--file", OSV_LOCKFILE])
}

/// A scanner that is not installed: skipped locally, a failure in CI (`FABRICA_CI=1`).
fn missing_scanner(tool: &str) -> GateResult {
    if ci_tier() {
        return Err(format!("{tool} not installed (required when {CI_TIER}=1)"));
    }
    println!("{tool}: skipped: {tool} not installed");
    Ok(())
}

/// Semgrep CE with the factory's own rules only; ERROR rules fail and `nosemgrep` comments are ignored.
/// Tests and scripts/ are not scanned.
fn semgrep_gate() -> GateResult {
    let Some(exe) = which("semgrep") else {
        return missing_scanner("semgrep");
    };
    run(
        &exe.to_string_lossy(),
        &[
            "scan",
            "--config",
            SEMGREP_RULES,
            "--error",
            "--severity",
            "ERROR",
            "--metrics",
            "off",
            "--disable-version-check",
            "--disable-nosem",
            "--quiet",
            "--exclude=scripts",
            "--exclude=tests",
            ".",
        ],
    )
}

/// The advisory groups with CVSS >= 7.0 or no score (unknown counts as high).
fn osv_blocking(report: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for result in report["results"].as_array().into_iter().flatten() {
        for pkg in result["packages"].as_array().into_iter().flatten() {
            let name = pkg["package"]["name"].as_str().unwrap_or_default();
            let version = pkg["package"]["version"].as_str().unwrap_or_default();
            for group in pkg["groups"].as_array().into_iter().flatten() {
                let severity = group["max_severity"].as_str().unwrap_or_default().trim();
                let score = severity.parse::<f64>().ok();
                if score.is_none_or(|s| s >= HIGH_CVSS) {
                    let ids: Vec<&str> = group["ids"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect();
                    let shown = if score.is_some() { severity } else { "unknown" };
                    out.push(format!("{name}@{version} {} ({shown})", ids.join(",")));
                }
            }
        }
    }
    out
}

/// Every crate in Cargo.lock against OSV; high or unknown severity fails.
fn osv_gate() -> GateResult {
    let Some(exe) = which("osv-scanner") else {
        return missing_scanner("osv-scanner");
    };
    let lockfile = format!("--lockfile={OSV_LOCKFILE}");
    let out = Command::new(exe)
        .args(["scan", "source", "--format", "json", lockfile.as_str()])
        .output()
        .map_err(|e| format!("osv-scanner failed: {e}"))?;
    eprint!("{}", String::from_utf8_lossy(&out.stderr));
    match out.status.code() {
        Some(0) => {
            println!("osv-scanner: no known vulnerabilities in {OSV_LOCKFILE}");
            return Ok(());
        }
        Some(OSV_NO_PACKAGES) => {
            println!("osv-scanner: no packages in {OSV_LOCKFILE}");
            return Ok(());
        }
        Some(1) => {}
        _ => return Err(format!("osv-scanner failed ({})", out.status)),
    }
    let report: Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("osv-scanner: unreadable JSON report: {e}"))?;
    let blocking = osv_blocking(&report);
    for line in &blocking {
        eprintln!("osv-scanner: {line}");
    }
    if !blocking.is_empty() {
        return Err(format!(
            "{} advisory group(s) of high or unknown severity",
            blocking.len()
        ));
    }
    println!("osv-scanner: only advisories below high severity (reported, not failing)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed from a real OSV-Scanner v2.6.0 report (`osv-scanner scan source --format json`); the unscored group
    // keeps the recorded shape with an empty max_severity.
    const REPORT: &str = r#"{"results": [{"packages": [
      {"package": {"name": "minimist", "version": "1.2.0"}, "groups": [
        {"ids": ["GHSA-vh95-rmgr-6w4m"], "max_severity": "5.6"},
        {"ids": ["GHSA-xvch-5gv4-984h"], "max_severity": "9.8"}]},
      {"package": {"name": "lodash", "version": "4.17.15"}, "groups": [
        {"ids": ["GHSA-29mw-wpgm-hmr9"], "max_severity": ""}]}]}]}"#;

    #[test]
    fn osv_blocks_high_and_unscored_advisories() {
        let report: Value = serde_json::from_str(REPORT).unwrap_or_default();
        assert_eq!(
            osv_blocking(&report),
            [
                "minimist@1.2.0 GHSA-xvch-5gv4-984h (9.8)",
                "lodash@4.17.15 GHSA-29mw-wpgm-hmr9 (unknown)"
            ]
        );
    }

    #[test]
    fn line_coverage_reads_the_llvm_cov_summary() {
        let report =
            r#"{"data": [{"totals": {"lines": {"count": 8, "covered": 7, "percent": 87.5}}}]}"#;
        assert_eq!(line_coverage(report), Ok(87.5));
        assert!(line_coverage("{}").is_err());
    }

    #[test]
    fn glob_segments_match_like_the_other_templates() {
        let matches = |p: &str, n: &str| {
            segment_matches(
                &p.chars().collect::<Vec<_>>(),
                &n.chars().collect::<Vec<_>>(),
            )
        };
        assert!(matches("[0-9][0-9][0-9][0-9]-*.md", "0001-record.md"));
        assert!(!matches("[0-9][0-9][0-9][0-9]-*.md", "001-record.md"));
        assert!(!matches("[0-9][0-9][0-9][0-9]-*.md", "0001-record.txt"));
        assert!(matches("a?c", "abc"));
        assert_eq!(count_lines("a\r\nb\n"), 2);
        assert_eq!(count_lines(""), 0);
    }
}
