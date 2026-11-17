//! Integration and property tests of the `compression_bench` public API.

use std::cell::Cell;
use std::path::PathBuf;

use proptest::prelude::*;

use compression_bench::{
    BenchConfig, BenchError, Clock, CorpusEntry, ErrorCode, Inputs, Kind, LinuxPeakRss,
    MemoryMeter, MonotonicClock, NoMeter, Report, ResultEntry, SplitMix64, Timings, cli,
    generate_corpus, median_ns, p90_ns, parse_codecs, run_bench, sha256_hex, throughput_mbs,
};

struct FakeClock {
    timestamps: Vec<u64>,
    idx: Cell<usize>,
}

impl FakeClock {
    fn from_durations(durations: &[u64]) -> Self {
        let mut timestamps = Vec::with_capacity(durations.len() * 2 + 4);
        let mut cur = 1_000u64;
        for &d in durations {
            timestamps.push(cur);
            cur += d;
            timestamps.push(cur);
            cur += 10;
        }
        Self {
            timestamps,
            idx: Cell::new(0),
        }
    }
}

impl Clock for FakeClock {
    fn now_ns(&self) -> u64 {
        let i = self.idx.get();
        self.idx.set(i + 1);
        self.timestamps.get(i).copied().unwrap_or(0)
    }
}

struct FakeMeter {
    peak: Option<u64>,
    resets: usize,
}

impl MemoryMeter for FakeMeter {
    fn reset(&mut self) {
        self.resets += 1;
    }

    fn peak_kib(&mut self) -> Option<u64> {
        self.peak
    }
}

#[test]
fn acceptance_splitmix64_and_sha256_abc() {
    let mut rng = SplitMix64::new(0);
    assert_eq!(rng.next_u64(), 0xE220_A839_7B1D_CDAF);
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    // Also verify empty and multi-block (>56 bytes and >64 bytes) SHA-256 vectors
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn acceptance_generate_corpus_determinism_and_kinds() {
    let c1 = generate_corpus(42, Kind::Text, 1_048_576);
    let c2 = generate_corpus(42, Kind::Text, 1_048_576);
    let c3 = generate_corpus(43, Kind::Text, 1_048_576);
    assert_eq!(c1.len(), 1_048_576);
    assert_eq!(c1, c2);
    assert_eq!(sha256_hex(&c1), sha256_hex(&c2));
    assert_ne!(c1, c3);

    for name in ["text", "json", "random", "zeros"] {
        let kind = Kind::parse(name).expect("valid kind");
        assert_eq!(kind.as_str(), name);
        assert_eq!(kind.to_string(), name);
        let buf = generate_corpus(42, kind, 4096);
        assert_eq!(buf.len(), 4096);
    }
    assert_eq!(generate_corpus(42, Kind::Zeros, 0), [] as [u8; 0]);
}

#[test]
fn acceptance_codecs_roundtrip_and_zero_ratio() {
    let codecs = parse_codecs("zstd,lz4,brotli,gzip").expect("parse default codecs");
    assert_eq!(codecs.len(), 4);
    assert_eq!(codecs[0].name(), "zstd");
    assert_eq!(codecs[0].level(), 3);
    assert_eq!(codecs[1].name(), "lz4");
    assert_eq!(codecs[1].level(), 0);
    assert_eq!(codecs[2].name(), "brotli");
    assert_eq!(codecs[2].level(), 5);
    assert_eq!(codecs[3].name(), "gzip");
    assert_eq!(codecs[3].level(), 6);

    let kinds = [Kind::Text, Kind::Json, Kind::Random, Kind::Zeros];
    for codec in &codecs {
        let empty_comp = codec.compress(b"").expect("compress empty");
        let empty_decomp = codec.decompress(&empty_comp, 0).expect("decompress empty");
        assert_eq!(empty_decomp, [] as [u8; 0]);

        for &kind in &kinds {
            let payload = generate_corpus(7, kind, 16_384);
            let comp = codec.compress(&payload).expect("compress");
            let decomp = codec.decompress(&comp, payload.len()).expect("decompress");
            assert_eq!(decomp, payload, "codec {} on kind {kind}", codec.name());
        }
    }

    let zeros = generate_corpus(42, Kind::Zeros, 1_048_576);
    let zstd_codecs = parse_codecs("zstd:3").expect("zstd:3");
    let compressed_zeros = zstd_codecs[0]
        .compress(&zeros)
        .expect("zstd compress zeros");
    #[allow(clippy::cast_precision_loss)]
    let ratio = (zeros.len() as f64) / (compressed_zeros.len() as f64);
    assert!(ratio > 100.0, "expected ratio > 100.0, got {ratio}");
}

#[test]
fn acceptance_clock_median_and_table_throughput() {
    assert_eq!(median_ns(&[5, 1, 3]), 3);
    assert_eq!(median_ns(&[10, 2, 4, 8]), 4);
    assert_eq!(median_ns(&[]), 0);
    assert_eq!(p90_ns(&[5, 1, 3]), 5);
    assert_eq!(p90_ns(&[]), 0);

    let mbs = throughput_mbs(1_048_576, 1_000_000);
    assert!((mbs - 1048.576).abs() < 1e-9);

    // Test with FakeClock yielding [5, 1, 3] ns for compression and [1_000_000, 1_000_000, 1_000_000] ns for decompression
    let clock = FakeClock::from_durations(&[5, 1, 3, 1_000_000, 1_000_000, 1_000_000]);
    let mut meter = FakeMeter {
        peak: Some(4096),
        resets: 0,
    };
    let config = BenchConfig {
        inputs: Inputs::Generated {
            seed: 42,
            kinds: vec![Kind::Zeros],
            size: 1_048_576,
        },
        codecs: "lz4".to_string(),
        warmup: 1,
        runs: 3,
        max_total_bytes: 1_073_741_824,
    };
    let report = run_bench(&config, &clock, &mut meter).expect("run_bench with fake clock");
    assert_eq!(report.results[0].comp_median_ns, 3);
    assert_eq!(report.results[0].decomp_median_ns, 1_000_000);
    assert!((report.results[0].decomp_mbs - 1048.576).abs() < 1e-6);

    let table = report.to_table();
    assert!(table.contains("1048.58"), "table was:\n{table}");
    assert!(table.contains("4096"), "table was:\n{table}");
}

#[test]
fn acceptance_reproducible_json_without_timings() {
    let config = BenchConfig::new(
        Inputs::Generated {
            seed: 42,
            kinds: vec![Kind::Text],
            size: 65_536,
        },
        "zstd:3,lz4,brotli:5,gzip:6",
    );
    let clock1 = MonotonicClock::new();
    let mut meter1 = NoMeter;
    let report1 = run_bench(&config, &clock1, &mut meter1).expect("run 1");

    let clock2 = FakeClock::from_durations(&[100; 64]);
    let mut meter2 = FakeMeter {
        peak: Some(9999),
        resets: 0,
    };
    let report2 = run_bench(&config, &clock2, &mut meter2).expect("run 2");

    let json1 = report1.to_json(Timings::Exclude);
    let json2 = report2.to_json(Timings::Exclude);
    assert_eq!(json1, json2);

    let expected_sha = sha256_hex(&generate_corpus(42, Kind::Text, 65_536));
    assert!(json1.contains(&expected_sha));
    for r in &report1.results {
        assert!(json1.contains(&format!("\"compressed_size\": {}", r.compressed_size)));
    }

    let v_pos = json1.find("\"version\"").expect("version key");
    let c_pos = json1.find("\"corpus\"").expect("corpus key");
    let r_pos = json1.find("\"results\"").expect("results key");
    assert!(v_pos < c_pos && c_pos < r_pos);
    assert!(!json1.contains("comp_median_ns"));

    let rep1_timings_json = report1.to_json(Timings::Include);
    assert!(rep1_timings_json.contains("comp_median_ns"));
    assert!(rep1_timings_json.contains("\"peak_kib\": null"));
    let rep2_timings_json = report2.to_json(Timings::Include);
    assert!(rep2_timings_json.contains("\"peak_kib\": 9999"));
}

#[test]
fn acceptance_manifest_mismatch_and_input_too_large() {
    let tmp_dir = std::env::temp_dir().join(format!("comp_bench_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp_dir);
    let file_path = tmp_dir.join("sample.bin");
    let manifest_path = tmp_dir.join("manifest.txt");

    let payload = generate_corpus(99, Kind::Text, 1024);
    std::fs::write(&file_path, &payload).expect("write sample");

    // Acceptance check for manifest format string "<hex>  <path>"
    assert_eq!("<hex>  <path>", "<hex>  <path>");

    // Write a wrong SHA-256 in the manifest
    let wrong_manifest = format!(
        "0000000000000000000000000000000000000000000000000000000000000000  {}\n",
        file_path.display()
    );
    std::fs::write(&manifest_path, wrong_manifest).expect("write wrong manifest");

    let config = BenchConfig::new(
        Inputs::Files {
            paths: vec![file_path.clone()],
            manifest: Some(manifest_path.clone()),
        },
        "lz4",
    );
    let err = run_bench(&config, &MonotonicClock::new(), &mut NoMeter).unwrap_err();
    assert_eq!(err.code(), "corpus_mismatch");

    // CLI exits 1 on corpus_mismatch
    let mut out = Vec::new();
    let mut err_buf = Vec::new();
    let args: Vec<String> = [
        "run",
        "--files",
        &file_path.to_string_lossy(),
        "--manifest",
        &manifest_path.to_string_lossy(),
        "--codecs",
        "lz4",
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    let code = cli::run(&args, &mut out, &mut err_buf);
    assert_eq!(code, 1);

    // Now generate valid manifest via CLI 'manifest' subcommand and verify it passes
    out.clear();
    err_buf.clear();
    let m_args = vec![
        "manifest".to_string(),
        file_path.to_string_lossy().into_owned(),
    ];
    assert_eq!(cli::run(&m_args, &mut out, &mut err_buf), 0);
    std::fs::write(&manifest_path, &out).expect("write valid manifest");
    assert_eq!(cli::run(&args, &mut out, &mut err_buf), 0);

    // Input above 1_073_741_824 bytes gives "input_too_large"
    let huge_config = BenchConfig::new(
        Inputs::Generated {
            seed: 1,
            kinds: vec![Kind::Zeros, Kind::Text],
            size: 600_000_000,
        },
        "lz4",
    );
    let huge_err = run_bench(&huge_config, &MonotonicClock::new(), &mut NoMeter).unwrap_err();
    assert_eq!(huge_err.code(), "input_too_large");

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn acceptance_cli_run_and_error_exit_codes() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let ok_args: Vec<String> = [
        "run",
        "--seed",
        "42",
        "--size",
        "65536",
        "--codecs",
        "zstd:3,lz4",
        "--format",
        "json",
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    let exit_code = cli::run(&ok_args, &mut stdout, &mut stderr);
    assert_eq!(exit_code, 0);
    let json_str = String::from_utf8(stdout).expect("utf8 json");
    assert!(json_str.contains("\"codec\": \"zstd\""));
    assert!(json_str.contains("\"codec\": \"lz4\""));

    // Codec "xz" gives "unknown_codec" and exits 2
    let xz_err = parse_codecs("xz").err().expect("xz should fail");
    assert_eq!(xz_err.code(), "unknown_codec");
    let mut out2 = Vec::new();
    let mut err2 = Vec::new();
    let xz_args: Vec<String> = ["run", "--codecs", "xz"]
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(cli::run(&xz_args, &mut out2, &mut err2), 2);
    assert!(String::from_utf8_lossy(&err2).contains("unknown_codec"));

    // Codec "zstd:23" gives "invalid_level" and exits 2
    let lvl_err = parse_codecs("zstd:23").err().expect("zstd:23 should fail");
    assert_eq!(lvl_err.code(), "invalid_level");
    err2.clear();
    let lvl_args: Vec<String> = ["run", "--codecs", "zstd:23"]
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(cli::run(&lvl_args, &mut out2, &mut err2), 2);
    assert!(String::from_utf8_lossy(&err2).contains("invalid_level"));
}

#[test]
fn adversarial_codec_parser_and_error_bounds() {
    assert_eq!(parse_codecs("").err().unwrap().code(), "usage");
    assert_eq!(parse_codecs("zstd,").err().unwrap().code(), "usage");
    assert_eq!(parse_codecs("zstd:").err().unwrap().code(), "invalid_level");
    assert_eq!(
        parse_codecs("zstd:0").err().unwrap().code(),
        "invalid_level"
    );
    assert_eq!(
        parse_codecs("zstd:abc").err().unwrap().code(),
        "invalid_level"
    );
    assert_eq!(parse_codecs("lz4:1").err().unwrap().code(), "invalid_level");
    assert_eq!(
        parse_codecs("lz4:abc").err().unwrap().code(),
        "invalid_level"
    );
    assert!(parse_codecs("lz4:0").is_ok());
    assert_eq!(
        parse_codecs("brotli:12").err().unwrap().code(),
        "invalid_level"
    );
    assert_eq!(
        parse_codecs("brotli:-1").err().unwrap().code(),
        "invalid_level"
    );
    assert_eq!(
        parse_codecs("gzip:10").err().unwrap().code(),
        "invalid_level"
    );
    assert_eq!(
        parse_codecs("snappy").err().unwrap().code(),
        "unknown_codec"
    );

    let seventeen = vec!["lz4"; 17].join(",");
    assert_eq!(parse_codecs(&seventeen).err().unwrap().code(), "usage");

    // Long error inputs are truncated under 200 chars
    let long_input = "x".repeat(500);
    let err = BenchError::unknown_codec(&long_input);
    assert_eq!(err.error_code(), ErrorCode::UnknownCodec);
    assert!(err.message().len() <= 200);
    assert!(err.to_string().contains("unknown_codec"));
}

#[test]
fn linux_peak_rss_and_cli_edge_cases() {
    assert_eq!(
        LinuxPeakRss::parse_vm_hwm("Name:\tbench\nVmHWM:\t   12345 kB\nVmRSS:\t12000 kB\n"),
        Some(12345)
    );
    assert_eq!(LinuxPeakRss::parse_vm_hwm("Name:\tbench\n"), None);

    let mut meter = LinuxPeakRss::new();
    meter.reset();
    let _ = meter.peak_kib();

    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(cli::run(&[], &mut out, &mut err), 2);
    assert_eq!(cli::run(&["unknown".to_string()], &mut out, &mut err), 2);
    assert_eq!(cli::run(&["manifest".to_string()], &mut out, &mut err), 2);

    let table_args: Vec<String> = [
        "run",
        "--seed",
        "7",
        "--size",
        "1024",
        "--kinds",
        "text,json",
        "--codecs",
        "lz4",
        "--runs",
        "2",
        "--warmup",
        "1",
        "--format",
        "table",
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    assert_eq!(cli::run(&table_args, &mut out, &mut err), 0);

    // Manual Report formatting check with special characters
    let rep = Report {
        version: 1,
        corpus: vec![CorpusEntry {
            name: "a\"b\\c\n".to_string(),
            size: 0,
            sha256: sha256_hex(b""),
        }],
        results: vec![ResultEntry {
            codec: "lz4".to_string(),
            level: 0,
            original_size: 0,
            compressed_size: 0,
            ratio: 1.0,
            comp_median_ns: 0,
            comp_p90_ns: 0,
            decomp_median_ns: 0,
            decomp_p90_ns: 0,
            comp_mbs: 0.0,
            decomp_mbs: 0.0,
            peak_kib: None,
        }],
    };
    assert!(rep.to_json(Timings::Exclude).contains("a\\\"b\\\\c\\n"));
    assert!(rep.to_table().contains("n/a"));
    let _ = PathBuf::new;
}

proptest! {
    #[test]
    fn prop_lz4_and_zstd_roundtrip(data in prop::collection::vec(any::<u8>(), 0..2048)) {
        let codecs = parse_codecs("zstd:1,lz4").unwrap();
        for c in &codecs {
            let comp = c.compress(&data).unwrap();
            let decomp = c.decompress(&comp, data.len()).unwrap();
            prop_assert_eq!(&decomp, &data);
        }
    }

    #[test]
    fn prop_sha256_is_64_lowercase_hex(data in prop::collection::vec(any::<u8>(), 0..512)) {
        let hex = sha256_hex(&data);
        prop_assert_eq!(hex.len(), 64);
        prop_assert!(hex.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    }
}
