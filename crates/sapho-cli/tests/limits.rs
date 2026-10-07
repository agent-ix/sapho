// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Case and byte ceilings for a Dataset of about fifteen thousand labelled cases.
use sapho_cli::*;
use sapho_core::*;
use sapho_evidence::{Case, Dataset, LabelKind, LabelProvenance, Split};
use sapho_graph::GraphSpec;
use sapho_recording::*;
use sapho_runtime::RunLimits;
use std::{
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};

const CASES: usize = 15_000;
/// The invocation the CLI guide documents for a Dataset of up to 20,000 cases.
const DOCUMENTED: [&str; 4] = ["--max-cases", "20000", "--max-artifact-bytes", "1073741824"];
/// A claim-sized body of text, so that a case is about as large as a short statement.
const CLAIM: &str = "The synthetic organisation reported that its measured output rose in the second period compared with the first, and that the change was recorded in the attached table without adjustment for seasonal variation.";
const CEILING: u64 = 1_073_741_824;

const GRAPH: &str = r#"
inputs: {text: {kind: text}}
nodes:
  - id: context
    operation: {kind: record}
    inputs: {text: {kind: input, name: text}}
  - id: questions
    operation:
      kind: questions
      questions:
        - id: q
          question: {kind: boolean, instructions: "Does the property hold?", yes: Holds, no: Absent}
  - id: ask
    operation: {kind: ask, backend: judge}
    inputs:
      state: {kind: node, node: context, port: result}
      questions: {kind: node, node: questions, port: result}
  - id: probability
    operation: {kind: probability, question: q, labels: ["true"]}
    inputs: {answers: {kind: node, node: ask, port: answers}}
outputs: {result: {kind: node, node: probability, port: result}}
"#;

/// The scripted backend: answers every question the same way and keeps what it answered.
struct Scripted {
    exchanges: Mutex<Vec<Exchange>>,
}
#[async_trait::async_trait]
impl ModelBackend for Scripted {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let response = ModelResponse {
            model: request.model.clone(),
            digest: None,
            raw: None,
            answers: request
                .questions
                .iter()
                .map(|q| {
                    (
                        q.id.clone(),
                        Answer::Boolean {
                            probability: Probability::new(0.8).unwrap(),
                        },
                    )
                })
                .collect(),
            usage: None,
        };
        self.exchanges.lock().unwrap().push(Exchange {
            request: request.clone(),
            response: response.clone(),
        });
        Ok(response)
    }
}

fn text_input(text: &str) -> Inputs {
    Inputs::from([(
        "text".into(),
        Datum::new("text", Value::Text(text.into())).unwrap(),
    )])
}
/// How much text a case carries: about 0.3 KB or about 1.8 KB per case in the Dataset file.
#[derive(Clone, Copy)]
enum Weight {
    Small,
    Large,
}
impl Weight {
    fn text(self, index: usize) -> String {
        match self {
            Self::Small => format!("Synthetic claim {index:05}. Output rose in the second period."),
            Self::Large => format!("Synthetic claim {index:05}. {}", CLAIM.repeat(8)),
        }
    }
}
fn case(index: usize, weight: Weight) -> Case {
    Case {
        id: ItemId::new(format!("case-{index:05}")).unwrap(),
        split: Split::Development,
        inputs: text_input(&weight.text(index)),
        labels: BTreeMap::from([("result".into(), !index.is_multiple_of(3))]),
        label_provenance: LabelProvenance {
            kind: LabelKind::Human,
            source: "curator".into(),
            model_digest: None,
            reference: "synthetic label".into(),
        },
    }
}
fn dataset(range: std::ops::Range<usize>, weight: Weight) -> Dataset {
    Dataset {
        id: SourceId::new("synthetic-large").unwrap(),
        cases: range.map(|index| case(index, weight)).collect(),
    }
}

/// Files shared by the tests of this binary; the recording holds every case's exchange.
struct Fixture {
    _root: tempfile::TempDir,
    graph: PathBuf,
    recording: PathBuf,
    large: PathBuf,
    root: PathBuf,
}
fn fixture(weight: Weight) -> &'static Fixture {
    static SMALL: OnceLock<Fixture> = OnceLock::new();
    static LARGE: OnceLock<Fixture> = OnceLock::new();
    let (cell, weight) = match weight {
        Weight::Small => (&SMALL, Weight::Small),
        Weight::Large => (&LARGE, Weight::Large),
    };
    cell.get_or_init(|| {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().to_path_buf();
        let graph = root.join("graph.yaml");
        std::fs::write(&graph, GRAPH).unwrap();
        let spec = GraphSpec::parse(GRAPH).unwrap();
        let scripted = Arc::new(Scripted {
            exchanges: Mutex::new(Vec::new()),
        });
        let mut registry = BackendRegistry::default();
        registry
            .register(
                BackendId::new("judge").unwrap(),
                BackendBinding {
                    backend: scripted.clone(),
                    model: "synthetic".into(),
                    expected_model: None,
                    distribution_policy: DistributionPolicy::Strict {},
                },
            )
            .unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let runner = Runner::new(&spec, &PrimitiveRegistry::default(), registry).unwrap();
        for index in 0..CASES {
            let report = runtime.block_on(runner.run(
                &case(index, weight).inputs,
                RunLimits::default(),
                None,
            ));
            assert!(report.error.is_none());
        }
        let recording = Recording {
            exchanges: std::mem::take(&mut *scripted.exchanges.lock().unwrap()),
        };
        assert_eq!(recording.exchanges.len(), CASES);
        let saved = root.join("recording.json");
        recording.write_new(&saved, 1 << 30).unwrap();
        let large = root.join("large.json");
        std::fs::write(
            &large,
            serde_json::to_vec(&dataset(0..CASES, weight)).unwrap(),
        )
        .unwrap();
        Fixture {
            _root: temporary,
            graph,
            recording: saved,
            large,
            root,
        }
    })
}

struct Outcome {
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: String,
}
fn run(arguments: &[&str]) -> Outcome {
    let output = Command::new(env!("CARGO_BIN_EXE_sapho"))
        .args(arguments)
        .env_clear()
        .output()
        .unwrap();
    Outcome {
        code: output.status.code(),
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}
fn json(outcome: &Outcome) -> serde_json::Value {
    serde_json::from_slice(&outcome.stdout).unwrap_or_else(|e| {
        panic!(
            "Invalid JSON: {e}; {} {}",
            outcome.stdout.len(),
            outcome.stderr
        )
    })
}
fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}
fn measure_arguments<'a>(
    fixture: &'a Fixture,
    dataset: &'a Path,
    extra: &[&'a str],
) -> Vec<&'a str> {
    let mut arguments = vec![
        "measure",
        text(&fixture.graph),
        "--dataset",
        text(dataset),
        "--replay",
        text(&fixture.recording),
        "--split",
        "development",
    ];
    arguments.extend_from_slice(extra);
    arguments
}

/// Trace: FR-056-AC-1
#[test]
fn measure_and_tune_complete_fifteen_thousand_cases_with_the_documented_flags() {
    let fixture = fixture(Weight::Small);
    let report = fixture.root.join("measure-report.json");
    let outcome = run(&measure_arguments(
        fixture,
        &fixture.large,
        &[&DOCUMENTED[..], &["--output", text(&report)]].concat(),
    ));
    assert_eq!(outcome.code, Some(0), "{}", outcome.stderr);
    // The streamed file and the streamed stdout carry the same document.
    let saved = std::fs::read(&report).unwrap();
    assert_eq!(&outcome.stdout[..saved.len()], saved.as_slice());
    assert_eq!(&outcome.stdout[saved.len()..], b"\n");
    let measured = json(&outcome);
    assert_eq!(measured["measurement"]["selected_cases"], CASES);
    assert_eq!(
        measured["measurement"]["outputs"]["result"]["scored"],
        CASES
    );
    assert_eq!(measured["runs"].as_array().unwrap().len(), CASES);

    let tuned_path = fixture.root.join("tune-report.json");
    let tuned = run(&[
        "tune",
        "--candidate",
        text(&fixture.graph),
        "--dataset",
        text(&fixture.large),
        "--replay",
        text(&fixture.recording),
        "--output-name",
        "result",
        "--metric",
        "brier",
        "--output",
        text(&tuned_path),
        DOCUMENTED[0],
        DOCUMENTED[1],
        DOCUMENTED[2],
        DOCUMENTED[3],
    ]);
    assert_eq!(tuned.code, Some(0), "{}", tuned.stderr);
    let tuned = json(&tuned);
    assert_eq!(
        tuned["candidates"][0]["measurement"]["selected_cases"],
        CASES
    );
    assert_eq!(tuned["ranking"].as_array().unwrap().len(), 1);
}

/// Trace: FR-056-AC-2
#[test]
fn the_dataset_is_refused_without_the_flags_and_a_file_above_a_lowered_ceiling_is_refused() {
    let fixture = fixture(Weight::Small);
    // The default case ceiling names 1024 and no model was called: the recording is never read.
    let missing = fixture.root.join("absent-recording.json");
    let outcome = run(&[
        "measure",
        text(&fixture.graph),
        "--dataset",
        text(&fixture.large),
        "--replay",
        text(&missing),
        "--split",
        "development",
        "--max-artifact-bytes",
        "1073741824",
    ]);
    assert_eq!(outcome.code, Some(2), "{}", outcome.stderr);
    let refusal = json(&outcome);
    assert_eq!(refusal["error"]["kind"], "evidence");
    assert_eq!(refusal["error"]["detail"]["kind"], "case_limit");
    assert_eq!(refusal["error"]["detail"]["detail"]["limit"], 1024);

    // A raised case ceiling still stops a larger Dataset, and nothing is cut to fit.
    let outcome = run(&measure_arguments(
        fixture,
        &fixture.large,
        &["--max-cases", "14999", "--max-artifact-bytes", "1073741824"],
    ));
    assert_eq!(outcome.code, Some(2));
    assert_eq!(json(&outcome)["error"]["detail"]["detail"]["limit"], 14999);

    // A file above a lowered byte ceiling is refused before it is parsed.
    let size = std::fs::metadata(&fixture.large).unwrap().len();
    let lowered = (size - 1).to_string();
    let outcome = run(&measure_arguments(
        fixture,
        &fixture.large,
        &["--max-cases", "20000", "--max-artifact-bytes", &lowered],
    ));
    assert_eq!(outcome.code, Some(2));
    let refusal = json(&outcome);
    assert_eq!(refusal["error"]["kind"], "engine");
    assert_eq!(refusal["error"]["detail"]["code"], "limit_exceeded");
    assert_eq!(
        refusal["error"]["detail"]["message"],
        "Input exceeds byte ceiling"
    );
    // The same applies to the other two commands that read a Dataset.
    let exported = fixture.root.join("training.jsonl");
    let outcome = run(&[
        "export-training",
        "--dataset",
        text(&fixture.large),
        "--output",
        text(&exported),
        "--max-cases",
        "1024",
        "--max-artifact-bytes",
        "1073741824",
    ]);
    assert_eq!(json(&outcome)["error"]["detail"]["detail"]["limit"], 1024);
    assert!(!exported.exists() || std::fs::metadata(&exported).unwrap().len() == 0);
}

/// Trace: FR-056-AC-3
#[test]
fn defaults_stay_at_1024_cases_and_8_mib_and_the_guide_documents_the_large_invocation() {
    for command in ["measure", "tune", "export-training"] {
        let help = run(&[command, "--help"]);
        let help = String::from_utf8(help.stdout).unwrap();
        assert!(help.contains("--max-cases"), "{command}");
        assert!(help.contains("[default: 1024]"), "{command}");
        assert!(help.contains("--max-artifact-bytes"), "{command}");
        assert!(help.contains("[default: 8388608]"), "{command}");
    }
    let guide = include_str!("../../../docs/cli-guide.md");
    assert!(guide.contains("--max-cases 20000"));
    assert!(guide.contains("--max-artifact-bytes 1073741824"));
    assert!(guide.contains("separately"));
}

/// Trace: FR-056-AC-4
#[test]
fn per_case_results_do_not_depend_on_how_cases_are_split_into_files() {
    let fixture = fixture(Weight::Small);
    let write = |name: &str, range: std::ops::Range<usize>| {
        let path = fixture.root.join(name);
        std::fs::write(
            &path,
            serde_json::to_vec(&dataset(range, Weight::Small)).unwrap(),
        )
        .unwrap();
        path
    };
    let (first, second, union) = (
        write("first.json", 0..600),
        write("second.json", 600..1200),
        write("union.json", 0..1200),
    );
    let measured = |path: &Path| {
        let outcome = run(&measure_arguments(
            fixture,
            path,
            &["--max-cases", "2000", "--max-artifact-bytes", "1073741824"],
        ));
        assert_eq!(outcome.code, Some(0), "{}", outcome.stderr);
        json(&outcome)
    };
    let (first, second, union) = (measured(&first), measured(&second), measured(&union));
    let runs = |report: &serde_json::Value| report["runs"].as_array().unwrap().clone();
    let separate: Vec<_> = runs(&first).into_iter().chain(runs(&second)).collect();
    let together = runs(&union);
    assert_eq!(separate.len(), 1200);
    assert_eq!(separate, together);
    let predictions =
        |report: &serde_json::Value| report["measurement"]["outputs"]["result"]["cases"].clone();
    let pieces = [predictions(&first), predictions(&second)]
        .iter()
        .flat_map(|cases| cases.as_array().cloned().unwrap_or_default())
        .collect::<Vec<_>>();
    let whole = predictions(&union).as_array().cloned().unwrap_or_default();
    assert_eq!(pieces, whole);
}

/// The peak resident memory of a run, as the operating system reports it for the process.
///
/// `/usr/bin/time` prints it: `-l` on macOS (bytes) and `-v` on GNU time (kilobytes). `None`
/// when that program is not installed.
fn peak_resident_bytes(arguments: &[&str], stdout: &Path) -> Option<u64> {
    let timer = Path::new("/usr/bin/time");
    if !timer.exists() {
        return None;
    }
    let (flag, unit) = if cfg!(target_os = "macos") {
        ("-l", 1)
    } else {
        ("-v", 1024)
    };
    let output = Command::new(timer)
        .arg(flag)
        .arg(env!("CARGO_BIN_EXE_sapho"))
        .args(arguments)
        .env_clear()
        .stdout(Stdio::from(File::create(stdout).unwrap()))
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    let report = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{report}");
    let peak = report
        .lines()
        .find_map(|line| {
            let line = line.trim();
            line.strip_suffix("maximum resident set size")
                .or_else(|| line.strip_prefix("Maximum resident set size (kbytes):"))
                .and_then(|number| number.trim().parse::<u64>().ok())
        })
        .unwrap_or_else(|| panic!("no peak in {report}"));
    Some(peak * unit)
}

/// Trace: FR-056-AC-5
#[test]
fn peak_memory_of_the_fifteen_thousand_case_measure_stays_within_its_bound_for_both_sizes() {
    for weight in [Weight::Small, Weight::Large] {
        let fixture = fixture(weight);
        let output = fixture.root.join("peak-report.json");
        let stdout = fixture.root.join("peak-stdout.json");
        let arguments = measure_arguments(
            fixture,
            &fixture.large,
            &[&DOCUMENTED[..], &["--output", text(&output)]].concat(),
        );
        let started = Instant::now();
        let Some(peak) = peak_resident_bytes(&arguments, &stdout) else {
            eprintln!("skipped: /usr/bin/time is not installed");
            return;
        };
        let elapsed = started.elapsed();
        let dataset_bytes = std::fs::metadata(&fixture.large).unwrap().len();
        let bound = 6 * dataset_bytes + 64 * 1_048_576 + CASES as u64 * 8 * 1024;
        eprintln!(
            "peak resident {peak} bytes, bound {bound}, dataset {dataset_bytes} bytes, report {} bytes, run {elapsed:?}",
            std::fs::metadata(&output).unwrap().len()
        );
        assert!(peak <= bound, "peak {peak} exceeds {bound}");
        for path in [&output, &stdout, &fixture.recording, &fixture.large] {
            assert!(std::fs::metadata(path).unwrap().len() <= CEILING);
        }
    }
}

/// Trace: FR-056-AC-2
#[test]
fn a_report_above_the_ceiling_on_stdout_alone_is_refused_without_a_partial_document() {
    let fixture = fixture(Weight::Small);
    let dataset_path = fixture.root.join("six-hundred.json");
    std::fs::write(
        &dataset_path,
        serde_json::to_vec(&dataset(0..600, Weight::Small)).unwrap(),
    )
    .unwrap();
    let inputs = std::fs::metadata(&dataset_path).unwrap().len();
    let mut recording: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&fixture.recording).unwrap()).unwrap();
    recording["exchanges"].as_array_mut().unwrap().truncate(600);
    let recording_path = fixture.root.join("six-hundred-recording.json");
    let recording_bytes = serde_json::to_vec(&recording).unwrap();
    std::fs::write(&recording_path, &recording_bytes).unwrap();
    // Every input fits the ceiling, and the report, many times the Dataset, does not.
    let ceiling = (inputs.max(recording_bytes.len() as u64) + 1).to_string();
    let outcome = run(&[
        "measure",
        text(&fixture.graph),
        "--dataset",
        text(&dataset_path),
        "--replay",
        text(&recording_path),
        "--split",
        "development",
        "--max-cases",
        "2000",
        "--max-artifact-bytes",
        &ceiling,
    ]);
    assert_eq!(outcome.code, Some(2), "{}", outcome.stderr);
    let refusal = json(&outcome);
    assert_eq!(refusal["error"]["detail"]["code"], "limit_exceeded");
    assert!(refusal.get("measurement").is_none());
    assert!(outcome.stdout.len() < 1000);
}

/// A recording of the first exchanges of the fixture, with one backend recorded under two models.
fn conflicting_recording(fixture: &Fixture) -> PathBuf {
    let mut recording: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&fixture.recording).unwrap()).unwrap();
    let exchanges = recording["exchanges"].as_array_mut().unwrap();
    exchanges.truncate(3);
    exchanges[1]["request"]["model"] = serde_json::json!("other");
    let path = fixture.root.join("conflicting.json");
    std::fs::write(&path, serde_json::to_vec(&recording).unwrap()).unwrap();
    path
}

/// Trace: FR-056-AC-6
#[test]
fn tune_keeps_per_candidate_errors_for_conflicting_recordings_and_differing_bindings() {
    let fixture = fixture(Weight::Small);
    let small = fixture.root.join("three.json");
    std::fs::write(
        &small,
        serde_json::to_vec(&dataset(0..3, Weight::Small)).unwrap(),
    )
    .unwrap();
    let conflicting = conflicting_recording(fixture);
    let differing = fixture.root.join("differing.yaml");
    std::fs::write(
        &differing,
        "judge: {provider: clm, model: different, distribution_policy: {kind: strict}}\n",
    )
    .unwrap();
    for (recording, bindings) in [(&conflicting, None), (&fixture.recording, Some(&differing))] {
        let mut arguments = vec![
            "tune",
            "--candidate",
            text(&fixture.graph),
            "--candidate",
            text(&fixture.graph),
            "--dataset",
            text(&small),
            "--replay",
            text(recording),
            "--output-name",
            "result",
            "--metric",
            "brier",
        ];
        if let Some(bindings) = bindings {
            arguments.extend(["--bindings", text(bindings)]);
        }
        let outcome = run(&arguments);
        assert_eq!(outcome.code, Some(2), "{}", outcome.stderr);
        let report = json(&outcome);
        // The ranking has nothing to rank; the failures are the candidates' own.
        assert_eq!(report["error"]["kind"], "no_candidates");
        let candidates = report["candidates"].as_array().unwrap();
        assert_eq!(candidates.len(), 2);
        for candidate in candidates {
            assert_eq!(candidate["error"]["detail"]["code"], "recording_mismatch");
            assert!(candidate["measurement"].is_null());
        }
        assert!(report["ranking"].as_array().unwrap().is_empty());
    }
}

/// Trace: FR-056-AC-6
#[test]
fn explicit_bindings_that_differ_from_the_recording_refuse_a_measure_and_a_replay() {
    let fixture = fixture(Weight::Small);
    let differing = fixture.root.join("differing-measure.yaml");
    std::fs::write(
        &differing,
        "judge: {provider: clm, model: different, distribution_policy: {kind: strict}}\n",
    )
    .unwrap();
    let small = fixture.root.join("one.json");
    std::fs::write(
        &small,
        serde_json::to_vec(&dataset(0..1, Weight::Small)).unwrap(),
    )
    .unwrap();
    let measured = run(&measure_arguments(
        fixture,
        &small,
        &["--bindings", text(&differing)],
    ));
    assert_eq!(measured.code, Some(2), "{}", measured.stderr);
    assert_eq!(
        json(&measured)["error"]["detail"]["code"],
        "recording_mismatch"
    );
    let input = fixture.root.join("input.json");
    std::fs::write(&input, br#"{"text":"x"}"#).unwrap();
    let replayed = run(&[
        "replay",
        text(&fixture.graph),
        "--input",
        text(&input),
        "--bindings",
        text(&differing),
        "--recording",
        text(&fixture.recording),
    ]);
    assert_eq!(replayed.code, Some(2));
    assert_eq!(
        json(&replayed)["error"]["detail"]["code"],
        "recording_mismatch"
    );
}

/// Trace: FR-056-AC-2
#[test]
fn a_streamed_artifact_above_the_ceiling_is_refused_before_anything_is_written() {
    let root = tempfile::tempdir().unwrap();
    let document = serde_json::json!({"items": [1, 2, 3], "name": "synthetic"});
    let exact = serde_json::to_vec(&document).unwrap();
    let written = root.path().join("exact.json");
    ArtifactWriter::create(&written)
        .unwrap()
        .finish_json(&document, exact.len())
        .unwrap();
    assert_eq!(std::fs::read(&written).unwrap(), exact);
    let refused = root.path().join("refused.json");
    let error = ArtifactWriter::create(&refused)
        .unwrap()
        .finish_json(&document, exact.len() - 1)
        .unwrap_err();
    assert!(matches!(
        error,
        CliError::Engine(error) if error.code == ErrorCode::LimitExceeded
    ));
    assert_eq!(std::fs::metadata(&refused).unwrap().len(), 0);
}
