use std::io::Write;
use std::path::PathBuf;

use crate::corpus::Kind;
use crate::error::{BenchError, ErrorCode};
use crate::harness::{
    BenchConfig, DEFAULT_MAX_TOTAL_BYTES, Inputs, LinuxPeakRss, MonotonicClock, Timings, run_bench,
};
use crate::sha256::sha256_hex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Table,
    Json,
}

/// Runs the `compression-bench` CLI with the given arguments and output streams.
///
/// Exit codes:
/// - `0`: Success
/// - `1`: Domain failure (`corpus_mismatch`, `roundtrip_failed`, `input_too_large`, `codec_error`)
/// - `2`: Usage, parse, or I/O error (`unknown_codec`, `invalid_level`, `usage`, `io`)
pub fn run(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match run_inner(args, stdout) {
        Ok(()) => 0,
        Err(err) => {
            let _ = writeln!(stderr, "error [{}]: {}", err.code(), err.message());
            match err.error_code() {
                ErrorCode::CorpusMismatch
                | ErrorCode::RoundtripFailed
                | ErrorCode::InputTooLarge
                | ErrorCode::CodecError => 1,
                ErrorCode::UnknownCodec
                | ErrorCode::InvalidLevel
                | ErrorCode::Io
                | ErrorCode::Usage => 2,
            }
        }
    }
}

fn run_inner(args: &[String], stdout: &mut dyn Write) -> Result<(), BenchError> {
    let Some(subcmd) = args.first().map(String::as_str) else {
        return Err(BenchError::usage(
            "missing command (expected 'run' or 'manifest')",
        ));
    };

    match subcmd {
        "run" => cmd_run(&args[1..], stdout),
        "manifest" => cmd_manifest(&args[1..], stdout),
        other => Err(BenchError::usage(&format!("unknown command: {other}"))),
    }
}

fn cmd_manifest(args: &[String], stdout: &mut dyn Write) -> Result<(), BenchError> {
    if args.is_empty() {
        return Err(BenchError::usage(
            "manifest requires at least one file path",
        ));
    }
    let mut total: usize = 0;
    for path_str in args {
        let meta =
            std::fs::metadata(path_str).map_err(|e| BenchError::io(&format!("{path_str}: {e}")))?;
        let len = usize::try_from(meta.len())
            .map_err(|_| BenchError::input_too_large(usize::MAX, DEFAULT_MAX_TOTAL_BYTES))?;
        total = total
            .checked_add(len)
            .ok_or_else(|| BenchError::input_too_large(usize::MAX, DEFAULT_MAX_TOTAL_BYTES))?;
        if total > DEFAULT_MAX_TOTAL_BYTES {
            return Err(BenchError::input_too_large(total, DEFAULT_MAX_TOTAL_BYTES));
        }
        let bytes =
            std::fs::read(path_str).map_err(|e| BenchError::io(&format!("{path_str}: {e}")))?;
        let digest = sha256_hex(&bytes);
        writeln!(stdout, "{digest}  {path_str}").map_err(|e| BenchError::io(&e.to_string()))?;
    }
    Ok(())
}

struct RunArgs {
    seed: Option<u64>,
    size: Option<usize>,
    kinds: Option<Vec<Kind>>,
    files: Vec<PathBuf>,
    manifest: Option<PathBuf>,
    codecs: Option<String>,
    runs: usize,
    warmup: usize,
    format: OutputFormat,
}

fn parse_run_flag(
    flag: &str,
    args: &[String],
    i: &mut usize,
    parsed: &mut RunArgs,
) -> Result<(), BenchError> {
    match flag {
        "--seed" => {
            *i += 1;
            let val = args
                .get(*i)
                .ok_or_else(|| BenchError::usage("--seed requires a value"))?;
            parsed.seed = Some(
                val.parse::<u64>()
                    .map_err(|_| BenchError::usage(&format!("invalid --seed: {val}")))?,
            );
            *i += 1;
        }
        "--size" => {
            *i += 1;
            let val = args
                .get(*i)
                .ok_or_else(|| BenchError::usage("--size requires a value"))?;
            parsed.size = Some(
                val.parse::<usize>()
                    .map_err(|_| BenchError::usage(&format!("invalid --size: {val}")))?,
            );
            *i += 1;
        }
        "--kinds" => {
            *i += 1;
            let val = args
                .get(*i)
                .ok_or_else(|| BenchError::usage("--kinds requires a value"))?;
            parsed.kinds = Some(Kind::parse_list(val)?);
            *i += 1;
        }
        "--files" => {
            *i += 1;
            while *i < args.len() && !args[*i].starts_with("--") {
                parsed.files.push(PathBuf::from(&args[*i]));
                *i += 1;
            }
            if parsed.files.is_empty() {
                return Err(BenchError::usage("--files requires at least one path"));
            }
        }
        "--manifest" => {
            *i += 1;
            let val = args
                .get(*i)
                .ok_or_else(|| BenchError::usage("--manifest requires a path"))?;
            parsed.manifest = Some(PathBuf::from(val));
            *i += 1;
        }
        "--codecs" => {
            *i += 1;
            let val = args
                .get(*i)
                .ok_or_else(|| BenchError::usage("--codecs requires a specification"))?;
            parsed.codecs = Some(val.clone());
            *i += 1;
        }
        "--runs" => {
            *i += 1;
            let val = args
                .get(*i)
                .ok_or_else(|| BenchError::usage("--runs requires a value"))?;
            parsed.runs = val
                .parse::<usize>()
                .map_err(|_| BenchError::usage(&format!("invalid --runs: {val}")))?;
            *i += 1;
        }
        "--warmup" => {
            *i += 1;
            let val = args
                .get(*i)
                .ok_or_else(|| BenchError::usage("--warmup requires a value"))?;
            parsed.warmup = val
                .parse::<usize>()
                .map_err(|_| BenchError::usage(&format!("invalid --warmup: {val}")))?;
            *i += 1;
        }
        "--format" => {
            *i += 1;
            let val = args
                .get(*i)
                .ok_or_else(|| BenchError::usage("--format requires 'table' or 'json'"))?;
            parsed.format = match val.as_str() {
                "table" => OutputFormat::Table,
                "json" => OutputFormat::Json,
                other => {
                    return Err(BenchError::usage(&format!(
                        "unknown --format '{other}' (expected 'table' or 'json')"
                    )));
                }
            };
            *i += 1;
        }
        other => {
            return Err(BenchError::usage(&format!("unexpected argument: {other}")));
        }
    }
    Ok(())
}

fn parse_run_args(args: &[String]) -> Result<RunArgs, BenchError> {
    let mut parsed = RunArgs {
        seed: None,
        size: None,
        kinds: None,
        files: Vec::new(),
        manifest: None,
        codecs: None,
        runs: 5,
        warmup: 1,
        format: OutputFormat::Table,
    };

    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        parse_run_flag(flag, args, &mut i, &mut parsed)?;
    }
    Ok(parsed)
}

fn resolve_inputs(
    args: RunArgs,
) -> Result<(Inputs, String, usize, usize, OutputFormat), BenchError> {
    let codecs_spec = args
        .codecs
        .ok_or_else(|| BenchError::usage("--codecs is required"))?;

    let inputs = if args.files.is_empty() {
        if args.manifest.is_some() {
            return Err(BenchError::usage("--manifest requires --files"));
        }
        Inputs::Generated {
            seed: args.seed.unwrap_or(42),
            kinds: args.kinds.unwrap_or_else(|| vec![Kind::Text]),
            size: args.size.unwrap_or(65_536),
        }
    } else {
        if args.seed.is_some() || args.size.is_some() || args.kinds.is_some() {
            return Err(BenchError::usage(
                "cannot combine --files with --seed, --size, or --kinds",
            ));
        }
        Inputs::Files {
            paths: args.files,
            manifest: args.manifest,
        }
    };

    Ok((inputs, codecs_spec, args.warmup, args.runs, args.format))
}

fn cmd_run(args: &[String], stdout: &mut dyn Write) -> Result<(), BenchError> {
    let parsed = parse_run_args(args)?;
    let (inputs, codecs_spec, warmup, runs, format) = resolve_inputs(parsed)?;

    let config = BenchConfig {
        inputs,
        codecs: codecs_spec,
        warmup,
        runs,
        max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
    };

    let clock = MonotonicClock::new();
    let mut meter = LinuxPeakRss::new();
    let report = run_bench(&config, &clock, &mut meter)?;

    match format {
        OutputFormat::Table => {
            write!(stdout, "{}", report.to_table()).map_err(|e| BenchError::io(&e.to_string()))?;
        }
        OutputFormat::Json => {
            write!(stdout, "{}", report.to_json(Timings::Include))
                .map_err(|e| BenchError::io(&e.to_string()))?;
        }
    }

    Ok(())
}
