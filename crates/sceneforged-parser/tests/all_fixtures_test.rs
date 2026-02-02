//! Comprehensive fixture tests for sceneforged-parser.
//!
//! This module tests the parser against fixture files from multiple sources:
//! - Manual test cases (movies, episodes, anime)
//! - parse-torrent-title (PTT) - MIT license
//! - go-parse-torrent-name - MIT license
//! - Sonarr test cases - extracted from GPL source

use sceneforged_parser::config::{ParserConfig, YearInTitleMode};
use sceneforged_parser::{parse, Parser};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// A single test case from a fixture file.
#[derive(Debug, Deserialize, Serialize)]
struct TestCase {
    input: String,
    expected: Expected,
}

/// Baseline snapshot for regression testing.
#[derive(Debug, Deserialize, Serialize)]
struct Baseline {
    version: u32,
    generated: String,
    total_passing: usize,
    passing_tests: Vec<PassingTest>,
}

/// A single passing test tracked in the baseline.
#[derive(Debug, Deserialize, Serialize, Clone)]
struct PassingTest {
    fixture: String,
    input: String,
}

/// Expected values for a test case.
#[derive(Debug, Deserialize, Serialize)]
struct Expected {
    title: Option<String>,
    year: Option<u16>,
    resolution: Option<String>,
    source: Option<String>,
    video_codec: Option<String>,
    audio_codec: Option<String>,
    audio_channels: Option<String>,
    release_group: Option<String>,
    seasons: Option<Vec<u16>>,
    episodes: Option<Vec<u16>>,
    #[serde(default)]
    streaming_service: Option<String>,
    #[serde(default)]
    hdr_format: Option<String>,
    #[serde(default)]
    bit_depth: Option<u8>,
    #[serde(default)]
    container: Option<String>,
    #[serde(default)]
    file_checksum: Option<String>,
}

/// Compare strings in a normalized way (case-insensitive, ignoring punctuation).
fn compare_normalized(actual: &Option<String>, expected: &str) -> bool {
    match actual {
        Some(a) => {
            if a == expected {
                return true;
            }
            if a.to_lowercase() == expected.to_lowercase() {
                return true;
            }
            let a_norm = a.to_lowercase().replace(['-', ' ', '.', '_'], "");
            let e_norm = expected.to_lowercase().replace(['-', ' ', '.', '_'], "");
            a_norm == e_norm
        }
        None => false,
    }
}

/// Result from running fixture tests.
struct FixtureResult {
    passed: usize,
    failed: usize,
    failures: Vec<String>,
    passing_inputs: Vec<String>,
}

/// Run all test cases from a fixture file using default config.
fn run_fixture_file(path: &str) -> FixtureResult {
    run_fixture_file_with_config(path, None)
}

/// Run all test cases from a fixture file with optional custom config.
fn run_fixture_file_with_config(path: &str, config: Option<&ParserConfig>) -> FixtureResult {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            println!("Warning: Could not read {}: {}", path, e);
            return FixtureResult {
                passed: 0,
                failed: 0,
                failures: vec![],
                passing_inputs: vec![],
            };
        }
    };

    let cases: Vec<TestCase> = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            println!("Warning: Could not parse {}: {}", path, e);
            return FixtureResult {
                passed: 0,
                failed: 0,
                failures: vec![],
                passing_inputs: vec![],
            };
        }
    };

    let parser = config.map(|c| Parser::new(c.clone()));

    let mut passed = 0;
    let mut failed = 0;
    let mut failures = Vec::new();
    let mut passing_inputs = Vec::new();

    for case in cases {
        let result = if let Some(ref p) = parser {
            p.parse(&case.input)
        } else {
            parse(&case.input)
        };
        let mut case_passed = true;
        let mut case_failures = Vec::new();

        // Check title
        if let Some(expected_title) = &case.expected.title {
            let actual_title = Some(result.title.value.as_str());
            if actual_title != Some(expected_title.as_str()) {
                // Try normalized comparison
                if !compare_normalized(&Some(result.title.value.clone()), expected_title) {
                    case_passed = false;
                    case_failures.push(format!(
                        "title: expected {:?}, got {:?}",
                        expected_title, result.title.value
                    ));
                }
            }
        }

        // Check year
        if let Some(expected_year) = case.expected.year {
            let actual_year = result.year.as_ref().map(|f| f.value);
            if actual_year != Some(expected_year) {
                case_passed = false;
                case_failures.push(format!(
                    "year: expected {:?}, got {:?}",
                    expected_year, actual_year
                ));
            }
        }

        // Check resolution
        if let Some(expected_res) = &case.expected.resolution {
            let actual = result.resolution.as_ref().map(|r| r.value.to_string());
            if !compare_normalized(&actual, expected_res) {
                case_passed = false;
                case_failures.push(format!(
                    "resolution: expected {:?}, got {:?}",
                    expected_res, actual
                ));
            }
        }

        // Check source
        if let Some(expected_source) = &case.expected.source {
            let actual = result.source.as_ref().map(|s| s.value.to_string());
            if !compare_normalized(&actual, expected_source) {
                case_passed = false;
                case_failures.push(format!(
                    "source: expected {:?}, got {:?}",
                    expected_source, actual
                ));
            }
        }

        // Check video encoder
        if let Some(expected_codec) = &case.expected.video_codec {
            let actual = result.video_encoder.as_ref().map(|c| c.value.to_string());
            if !compare_normalized(&actual, expected_codec) {
                case_passed = false;
                case_failures.push(format!(
                    "video_encoder: expected {:?}, got {:?}",
                    expected_codec, actual
                ));
            }
        }

        // Check release group
        if let Some(expected_group) = &case.expected.release_group {
            let actual_group = result.release_group.as_ref().map(|g| g.value.clone());
            if !compare_normalized(&actual_group, expected_group) {
                case_passed = false;
                case_failures.push(format!(
                    "release_group: expected {:?}, got {:?}",
                    expected_group, actual_group
                ));
            }
        }

        // Check seasons
        if let Some(expected_seasons) = &case.expected.seasons {
            let actual_seasons: Vec<u16> = result.seasons.iter().map(|f| f.value).collect();
            if &actual_seasons != expected_seasons {
                case_passed = false;
                case_failures.push(format!(
                    "seasons: expected {:?}, got {:?}",
                    expected_seasons, actual_seasons
                ));
            }
        }

        // Check episodes
        if let Some(expected_episodes) = &case.expected.episodes {
            let actual_episodes: Vec<u16> = result.episodes.iter().map(|f| f.value).collect();
            if &actual_episodes != expected_episodes {
                case_passed = false;
                case_failures.push(format!(
                    "episodes: expected {:?}, got {:?}",
                    expected_episodes, actual_episodes
                ));
            }
        }

        if case_passed {
            passed += 1;
            passing_inputs.push(case.input.clone());
        } else {
            failed += 1;
            if failures.len() < 20 {
                failures.push(format!(
                    "FAIL: {}\n  {}",
                    case.input,
                    case_failures.join("\n  ")
                ));
            }
        }
    }

    FixtureResult {
        passed,
        failed,
        failures,
        passing_inputs,
    }
}

/// Config for manual fixtures: years before seasons are metadata, not part of title.
/// This matches expectations like "Doctor Who" (2005) where year disambiguates.
fn manual_fixtures_config() -> ParserConfig {
    ParserConfig::builder()
        .year_in_title(YearInTitleMode::TreatAsMetadata)
        .build()
}

#[test]
fn test_manual_movies() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/movies.json");
    if Path::new(path).exists() {
        let config = manual_fixtures_config();
        let result = run_fixture_file_with_config(path, Some(&config));
        println!("\n=== Manual Movie Fixtures ===");
        println!("{} passed, {} failed", result.passed, result.failed);
        for f in result.failures.iter().take(5) {
            println!("{}", f);
        }
    }
}

#[test]
fn test_manual_episodes() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/episodes.json");
    if Path::new(path).exists() {
        let config = manual_fixtures_config();
        let result = run_fixture_file_with_config(path, Some(&config));
        println!("\n=== Manual Episode Fixtures ===");
        println!("{} passed, {} failed", result.passed, result.failed);
        for f in result.failures.iter().take(5) {
            println!("{}", f);
        }
    }
}

#[test]
fn test_manual_anime() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/anime.json");
    if Path::new(path).exists() {
        let config = manual_fixtures_config();
        let result = run_fixture_file_with_config(path, Some(&config));
        println!("\n=== Manual Anime Fixtures ===");
        println!("{} passed, {} failed", result.passed, result.failed);
        for f in result.failures.iter().take(5) {
            println!("{}", f);
        }
    }
}

#[test]
fn test_sonarr_cases() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/sonarr_cases.json"
    );
    if Path::new(path).exists() {
        let result = run_fixture_file(path);
        let total = result.passed + result.failed;
        println!("\n=== Sonarr Fixtures (extracted) ===");
        println!(
            "{} passed, {} failed ({}%)",
            result.passed,
            result.failed,
            if total > 0 {
                result.passed * 100 / total
            } else {
                0
            }
        );
        for f in result.failures.iter().take(10) {
            println!("{}", f);
        }
    }
}

/// Fixture metadata for test running.
const FIXTURES: &[(&str, &str)] = &[
    (
        "movies.json",
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/movies.json"),
    ),
    (
        "episodes.json",
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/episodes.json"),
    ),
    (
        "anime.json",
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/anime.json"),
    ),
    (
        "sonarr_cases.json",
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/sonarr_cases.json"
        ),
    ),
];

/// Summary test that reports overall statistics.
#[test]
fn test_fixture_summary() {
    let display_names = ["Manual Movies", "Manual Episodes", "Manual Anime", "Sonarr"];

    // Manual fixtures use TreatAsMetadata, Sonarr uses IncludeInTitle (default)
    let manual_config = manual_fixtures_config();
    let configs: [Option<&ParserConfig>; 4] = [
        Some(&manual_config), // movies
        Some(&manual_config), // episodes
        Some(&manual_config), // anime
        None,                 // sonarr (default)
    ];

    let mut total_passed = 0;
    let mut total_failed = 0;

    println!("\n========== FIXTURE TEST SUMMARY ==========\n");

    for (i, (_, path)) in FIXTURES.iter().enumerate() {
        if Path::new(path).exists() {
            let result = run_fixture_file_with_config(path, configs[i]);
            let total = result.passed + result.failed;
            let rate = if total > 0 {
                result.passed * 100 / total
            } else {
                0
            };
            println!(
                "{:20} {:4}/{:4} ({:3}%)",
                display_names[i], result.passed, total, rate
            );
            total_passed += result.passed;
            total_failed += result.failed;
        }
    }

    let total = total_passed + total_failed;
    let overall_rate = if total > 0 {
        total_passed * 100 / total
    } else {
        0
    };
    println!("----------------------------------------");
    println!(
        "{:20} {:4}/{:4} ({:3}%)",
        "TOTAL", total_passed, total, overall_rate
    );
    println!("\n==========================================");
}

/// Get the appropriate config for a fixture by name.
fn config_for_fixture(fixture_name: &str) -> Option<ParserConfig> {
    match fixture_name {
        "movies.json" | "episodes.json" | "anime.json" => Some(manual_fixtures_config()),
        "sonarr_cases.json" => None, // Use default (IncludeInTitle)
        _ => None,
    }
}

/// Regression test - ensures no previously passing tests have regressed.
#[test]
fn test_no_regressions() {
    let baseline_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/baseline.json");

    if !Path::new(baseline_path).exists() {
        println!(
            "No baseline file found at {} - skipping regression check",
            baseline_path
        );
        println!("Run `cargo test -- --ignored generate_baseline` to create one.");
        return;
    }

    let baseline_content = fs::read_to_string(baseline_path).expect("Failed to read baseline file");
    let baseline: Baseline =
        serde_json::from_str(&baseline_content).expect("Failed to parse baseline file");

    // Build a lookup of passing tests per fixture
    let mut fixture_results: std::collections::HashMap<String, std::collections::HashSet<String>> =
        std::collections::HashMap::new();

    for (fixture_name, path) in FIXTURES {
        if Path::new(path).exists() {
            let config = config_for_fixture(fixture_name);
            let result = run_fixture_file_with_config(path, config.as_ref());
            let passing_set: std::collections::HashSet<String> =
                result.passing_inputs.into_iter().collect();
            fixture_results.insert(fixture_name.to_string(), passing_set);
        }
    }

    // Check each baseline test
    let mut regressions = Vec::new();
    for test in &baseline.passing_tests {
        if let Some(passing_set) = fixture_results.get(&test.fixture) {
            if !passing_set.contains(&test.input) {
                regressions.push(format!("[{}] {}", test.fixture, test.input));
            }
        }
    }

    if !regressions.is_empty() {
        let sample: Vec<_> = regressions.iter().take(20).collect();
        panic!(
            "\n\n🚨 REGRESSION DETECTED: {} tests that previously passed now fail:\n{}\n\n\
             If these changes are intentional, update the baseline with:\n\
             cargo test -- --ignored generate_baseline --nocapture\n",
            regressions.len(),
            sample
                .iter()
                .map(|s| format!("  - {}", s))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    println!(
        "✅ No regressions detected ({} baseline tests still passing)",
        baseline.total_passing
    );
}

/// Generate a new baseline snapshot from currently passing tests.
/// Run with: cargo test -- --ignored generate_baseline --nocapture
#[test]
#[ignore]
fn generate_baseline() {
    let mut all_passing = Vec::new();

    for (fixture_name, path) in FIXTURES {
        if Path::new(path).exists() {
            let config = config_for_fixture(fixture_name);
            let result = run_fixture_file_with_config(path, config.as_ref());
            for input in result.passing_inputs {
                all_passing.push(PassingTest {
                    fixture: fixture_name.to_string(),
                    input,
                });
            }
            println!(
                "Collected {} passing tests from {}",
                all_passing.len()
                    - all_passing
                        .iter()
                        .filter(|t| t.fixture != *fixture_name)
                        .count(),
                fixture_name
            );
        }
    }

    let baseline = Baseline {
        version: 1,
        generated: chrono::Utc::now().to_rfc3339(),
        total_passing: all_passing.len(),
        passing_tests: all_passing,
    };

    let baseline_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/baseline.json");
    fs::write(
        baseline_path,
        serde_json::to_string_pretty(&baseline).expect("Failed to serialize baseline"),
    )
    .expect("Failed to write baseline file");

    println!(
        "\n✅ Generated baseline with {} passing tests at:\n   {}",
        baseline.total_passing, baseline_path
    );
}
