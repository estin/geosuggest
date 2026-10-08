//! Simple memory + performance benchmark for the index.
//!
//! Run: `cargo run -p geosuggest-core --example bench --release`
//! Prints archive size, bytes per city/entry, loader capacity waste,
//! RSS high-water mark, and suggest/reverse latency. Save the output,
//! change the layout, re-run, and compare the two runs.
use geosuggest_core::{
    index::{IndexData, SourceFileContentOptions, SourceFileOptions},
    storage::Storage,
    EngineData,
};
use std::time::Instant;

fn manifest_path(name: &str) -> String {
    format!("{}/tests/misc/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// RSS high-water mark in KiB (Linux only, 0 elsewhere).
fn rss_hwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines().find(|l| l.starts_with("VmHWM:")).and_then(|l| {
                l.split_whitespace().nth(1)?.parse::<u64>().ok()
            })
        })
        .unwrap_or(0)
}

fn report(tag: &str, data: &EngineData) {
    let engine = data.as_engine().expect("as_engine");
    let cities = engine.data.geonames.len();
    let entries = engine.data.entries.len();
    let len = data.data.len();
    let cap = data.data.capacity();
    println!("== {tag} ==");
    println!("cities={cities} entries={entries}");
    println!("archive_len={len} archive_capacity={cap}");
    println!(
        "capacity_waste_pct={:.1}",
        if len > 0 {
            (cap - len) as f64 / len as f64 * 100.0
        } else {
            0.0
        }
    );
    if cities > 0 {
        println!("bytes_per_city={:.1}", len as f64 / cities as f64);
    }
    if entries > 0 {
        println!("bytes_per_entry={:.2}", len as f64 / entries as f64);
    }
}

/// Time `iters` runs of `f`, return average microseconds per run.
fn bench_avg_us<F: FnMut()>(mut f: F, iters: usize, warmup: usize) -> f64 {
    for _ in 0..warmup {
        f();
    }
    let now = Instant::now();
    for _ in 0..iters {
        f();
    }
    now.elapsed().as_micros() as f64 / iters as f64
}

fn bench_queries(tag: &str, data: &EngineData, suggest_iters: usize, reverse_iters: usize) {
    let engine = data.as_engine().expect("as_engine");
    // one common-prefix query (scans everything) and one selective query
    let common = bench_avg_us(
        || {
            let _ = engine.suggest::<&str>("city", 10, None, None);
        },
        suggest_iters,
        2,
    );
    let selective = bench_avg_us(
        || {
            let _ = engine.suggest::<&str>("city01234", 10, None, None);
        },
        suggest_iters,
        2,
    );
    let reverse = bench_avg_us(
        || {
            let _ = engine.reverse::<&str>((51.5, -0.12), 10, None, None);
        },
        reverse_iters,
        2,
    );
    println!("{tag}_suggest_common_us={common:.1} qps={:.0}", 1_000_000.0 / common);
    println!(
        "{tag}_suggest_selective_us={selective:.1} qps={:.0}",
        1_000_000.0 / selective
    );
    println!("{tag}_reverse_us={reverse:.1} qps={:.0}", 1_000_000.0 / reverse);
}

fn bench_filtered_reverse(data: &EngineData, code: &str, iters: usize) {
    let engine = data.as_engine().expect("as_engine");
    let avg = bench_avg_us(
        || {
            let _ = engine.reverse((51.6372, 39.1937), 10, None, Some(&[code]));
        },
        iters,
        2,
    );
    println!("filtered_reverse_{code}_us={avg:.1} qps={:.0}", 1_000_000.0 / avg);
}

fn build_fixture() -> EngineData {
    let now = Instant::now();
    let index = IndexData::new_from_files(SourceFileOptions {
        cities: manifest_path("cities.txt"),
        names: Some(manifest_path("names.txt")),
        countries: Some(manifest_path("country-info.txt")),
        admin1_codes: Some(manifest_path("admin1-codes.txt")),
        admin2_codes: Some(manifest_path("admin2-codes.txt")),
        filter_languages: vec!["en", "de"],
        excluded_feature_codes: geosuggest_core::index::DEFAULT_EXCLUDED_FEATURE_CODES.to_vec(),
    })
    .expect("fixture index");
    let build_ms = now.elapsed().as_millis();
    let now = Instant::now();
    let data = EngineData::try_from(index).expect("serialize");
    println!("fixture_build_ms={build_ms} fixture_serialize_ms={}", now.elapsed().as_millis());
    data
}

/// Deterministic synthetic cities table with `n` rows (no countries).
fn synthetic_cities(n: usize) -> String {
    let mut s = String::with_capacity(n * 128);
    for i in 0..n {
        let id = 1_000_000 + i;
        let lat = 30.0 + (i % 4000) as f64 / 100.0;
        let lon = -120.0 + (i % 6000) as f64 / 100.0;
        // every 1000th city is a capital, to keep the translated-names path alive
        let fcode = if i % 1000 == 0 { "PPLC" } else { "PPL" };
        // geonameid name asciiname alternatenames lat lon fclass fcode cc cc2 admin1 admin2 admin3 admin4 pop elev dem tz mdate
        s.push_str(&format!(
            "{id}\tcity{i:05}\tcity{i:05}\talt{i}a,alt{i}b\t{lat:.4}\t{lon:.4}\tP\t{fcode}\tUS\t\tCA\t\t\t\t{pop}\t\t\tAmerica/Los_Angeles\t2020-01-01\n",
            pop = 1000 + (i % 500_000)
        ));
    }
    s
}

/// Deterministic synthetic alternate-names table: en (preferred) + de per city.
/// `extra_skipped` adds rows no city/country/admin uses, to exercise rejection.
fn synthetic_names(n: usize, extra_skipped: usize) -> String {
    let mut s = String::with_capacity((n * 2 + extra_skipped) * 48);
    for i in 0..n {
        let id = 1_000_000 + i;
        let altid = 2_000_000 + i;
        s.push_str(&format!("{altid}\t{id}\ten\tcity{i:05}\t1\t0\t0\t0\t\t\n"));
        let altid = 3_000_000 + i;
        s.push_str(&format!("{altid}\t{id}\tde\tstadt{i:05}\t0\t0\t0\t0\t\t\n"));
    }
    for i in 0..extra_skipped {
        let id = 90_000_000 + i;
        let lang = if i % 3 == 0 { "xx" } else { "en" };
        s.push_str(&format!("{i}\t{id}\t{lang}\tghost{i}\t0\t0\t0\t0\t\t\n"));
    }
    s
}

fn build_synthetic(n: usize, extra_skipped_names: usize) -> EngineData {
    let now = Instant::now();
    let cities = synthetic_cities(n);
    let names = synthetic_names(n, extra_skipped_names);
    println!(
        "synthetic_input_bytes={} (cities) + {} (names)",
        cities.len(),
        names.len()
    );
    let index = IndexData::new_from_files_content(SourceFileContentOptions {
        cities,
        names: Some(names),
        countries: None,
        admin1_codes: None,
        admin2_codes: None,
        filter_languages: vec!["en", "de"],
        excluded_feature_codes: geosuggest_core::index::DEFAULT_EXCLUDED_FEATURE_CODES.to_vec(),
    })
    .expect("synthetic index");
    let build_ms = now.elapsed().as_millis();
    let now = Instant::now();
    let data = EngineData::try_from(index).expect("serialize");
    println!("synthetic_build_ms={build_ms} synthetic_serialize_ms={}", now.elapsed().as_millis());
    data
}

fn roundtrip(tag: &str, data: &EngineData, probe: &str) {
    let path = std::env::temp_dir().join(format!("geosuggest-bench-{tag}.rkyv"));
    let storage = Storage::new();
    let now = Instant::now();
    storage.dump_to(&path, data).expect("dump");
    let dump_ms = now.elapsed().as_millis();
    let file_len = std::fs::metadata(&path).expect("stat").len();
    let now = Instant::now();
    let loaded = storage.load_from(&path).expect("load");
    let load_ms = now.elapsed().as_millis();
    println!("{tag}_dump_ms={dump_ms} {tag}_file_len={file_len}");
    println!("{tag}_load_ms={load_ms}");
    println!(
        "{tag}_loaded_len={} {tag}_loaded_capacity={} {tag}_loaded_waste_pct={:.1}",
        loaded.data.len(),
        loaded.data.capacity(),
        if !loaded.data.is_empty() {
            (loaded.data.capacity() - loaded.data.len()) as f64 / loaded.data.len() as f64 * 100.0
        } else {
            0.0
        }
    );
    // correctness spot check after roundtrip
    let engine = loaded.as_engine().expect("as_engine");
    let before = data.as_engine().expect("as_engine");
    assert_eq!(engine.data.geonames.len(), before.data.geonames.len());
    assert_eq!(engine.data.entries.len(), before.data.entries.len());
    assert!(!engine.suggest::<&str>(probe, 1, None, None).is_empty());
    let _ = std::fs::remove_file(&path);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(20_000);
    let extra_skipped: usize = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(0);

    println!("rss_hwm_start_kb={}", rss_hwm_kb());
    let fixture = build_fixture();
    println!("rss_hwm_after_fixture_build_kb={}", rss_hwm_kb());
    report("fixture", &fixture);
    roundtrip("fixture", &fixture, "voronezh");
    bench_queries("fixture", &fixture, 500, 200);
    bench_filtered_reverse(&fixture, "GB", 200);

    let synth = build_synthetic(n, extra_skipped);
    println!("rss_hwm_after_synthetic_build_kb={}", rss_hwm_kb());
    report("synthetic", &synth);
    roundtrip("synthetic", &synth, "city");
    bench_queries("synthetic", &synth, 30, 100);

    println!("rss_hwm_end_kb={}", rss_hwm_kb());
}
