// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Bounded literal-only Cartesian generation with exclusive publication.
use crate::{CliError, read_bytes, select_format};
use sapho_core::{
    ErrorCode, PrimitiveRegistry, Probability, SaphoError, Value, ValueType, decode_json,
};
use sapho_graph::{Binding, GraphFormat, GraphSpec, NodeSpec};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

const INPUT_BYTES: usize = 1_048_576;
const CANDIDATE_BYTES: usize = 1_048_576;
const TOTAL_BYTES: usize = 32 * 1_048_576;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Grid {
    axes: Vec<Axis>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Axis {
    id: String,
    values: Vec<JsonValue>,
}
struct CheckedAxis {
    id: String,
    values: Vec<(JsonValue, Value)>,
}
#[derive(Serialize)]
struct Coordinate<'a> {
    id: &'a str,
    value: &'a JsonValue,
}
#[derive(Serialize)]
struct ManifestEntry<'a> {
    index: usize,
    file: String,
    coordinates: Vec<Coordinate<'a>>,
    sha256: String,
}
#[derive(Serialize)]
struct Manifest<'a> {
    format: &'static str,
    base_sha256: String,
    grid_sha256: String,
    entries: Vec<ManifestEntry<'a>>,
}
struct Plan {
    files: Vec<(String, Vec<u8>)>,
    manifest: Vec<u8>,
}

/// Completed file publication; no model, dataset or recording is opened.
#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SweepReport {
    /// Newly created output directory.
    pub output_dir: PathBuf,
    /// Count of explicit candidate GraphSpec files.
    pub candidates: usize,
    /// Versioned manifest path.
    pub manifest: PathBuf,
}
fn config(message: &'static str) -> CliError {
    SaphoError::new(ErrorCode::Config, message).into()
}
fn limit(message: &'static str) -> CliError {
    SaphoError::new(ErrorCode::LimitExceeded, message).into()
}
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn io(path: &Path, source: std::io::Error) -> CliError {
    CliError::Io {
        path: path.into(),
        source,
    }
}

fn walk_nodes(
    nodes: &mut [NodeSpec],
    outputs: &mut BTreeMap<String, Binding>,
    f: &mut impl FnMut(&mut Binding),
) {
    for node in nodes {
        for binding in node.inputs.values_mut() {
            f(binding);
        }
        if let Some(guard) = node.guard.as_mut() {
            f(guard);
        }
    }
    for binding in outputs.values_mut() {
        f(binding);
    }
}
fn walk(graph: &mut GraphSpec, f: &mut impl FnMut(&mut Binding)) {
    walk_nodes(&mut graph.nodes, &mut graph.outputs, f);
    for body in graph.subgraphs.values_mut() {
        walk_nodes(&mut body.nodes, &mut body.outputs, f);
    }
}
fn target(base: &GraphSpec, id: &str) -> Result<(Value, ValueType), CliError> {
    let mut graph = base.clone();
    let mut found = Vec::new();
    walk(&mut graph, &mut |binding| {
        if let Binding::Literal { value, value_type } = binding
            && value.id.as_str() == id
        {
            found.push((value.value.clone(), value_type.clone()));
        }
    });
    if found.len() != 1 {
        return Err(config("Axis ID must name exactly one literal occurrence"));
    }
    let (value, ty) = found.remove(0);
    ty.check(&value)?;
    if ty != ValueType::Probability && ty != ValueType::list(ValueType::Number) {
        return Err(config("Axis literal must be Probability or List(Number)"));
    }
    Ok((value, ty))
}
fn replacement(raw: &JsonValue, base: &Value, ty: &ValueType) -> Result<Value, CliError> {
    let value = match (base, ty) {
        (Value::Probability(_), ValueType::Probability) => {
            let number = raw
                .as_f64()
                .filter(|n| n.is_finite())
                .ok_or_else(|| config("Probability grid value must be finite JSON number"))?;
            Value::Probability(Probability::new(number)?)
        }
        (Value::List(original), ValueType::List { item })
            if item.as_ref() == &ValueType::Number =>
        {
            let numbers = raw
                .as_array()
                .filter(|items| !items.is_empty() && items.len() == original.len())
                .ok_or_else(|| config("Weight vector length must match nonempty base list"))?;
            let mut copied = original.clone();
            for (datum, raw_number) in copied.iter_mut().zip(numbers) {
                let number = raw_number
                    .as_f64()
                    .filter(|n| n.is_finite())
                    .ok_or_else(|| config("Weight must be finite JSON number"))?;
                datum.value = Value::Number(number);
            }
            Value::List(copied)
        }
        _ => return Err(config("Axis literal has unsupported value")),
    };
    ty.check(&value)?;
    Ok(value)
}
fn checked_axes(
    base: &GraphSpec,
    bytes: &[u8],
    maximum: usize,
) -> Result<(Vec<CheckedAxis>, usize), CliError> {
    if !(1..=256).contains(&maximum) {
        return Err(limit("Sweep ceiling must be 1 through 256"));
    }
    let grid: Grid = decode_json(bytes, INPUT_BYTES)?;
    if !(1..=8).contains(&grid.axes.len()) {
        return Err(config("Sweep needs one to eight axes"));
    }
    let mut seen = BTreeSet::new();
    let mut product = 1usize;
    let mut axes = Vec::with_capacity(grid.axes.len());
    for axis in grid.axes {
        if axis.id.is_empty() || !seen.insert(axis.id.clone()) {
            return Err(config("Axis IDs must be nonempty and unique"));
        }
        if !(1..=16).contains(&axis.values.len()) {
            return Err(config("Each axis needs one to 16 values"));
        }
        product = product
            .checked_mul(axis.values.len())
            .ok_or_else(|| limit("Sweep product overflow"))?;
        if product > maximum {
            return Err(limit("Sweep product exceeds candidate ceiling"));
        }
        let (base_value, ty) = target(base, &axis.id)?;
        let values = axis
            .values
            .into_iter()
            .map(|raw| {
                let typed = replacement(&raw, &base_value, &ty)?;
                Ok((raw, typed))
            })
            .collect::<Result<Vec<_>, CliError>>()?;
        axes.push(CheckedAxis {
            id: axis.id,
            values,
        });
    }
    Ok((axes, product))
}
fn coordinate(mut index: usize, axes: &[CheckedAxis]) -> Vec<usize> {
    let mut positions = vec![0; axes.len()];
    for axis in (0..axes.len()).rev() {
        positions[axis] = index % axes[axis].values.len();
        index /= axes[axis].values.len();
    }
    positions
}
fn plan(
    base: &GraphSpec,
    base_bytes: &[u8],
    grid_bytes: &[u8],
    maximum: usize,
) -> Result<Plan, CliError> {
    crate::inspect(base, &PrimitiveRegistry::default())?;
    let (axes, count) = checked_axes(base, grid_bytes, maximum)?;
    let mut files = Vec::with_capacity(count);
    let mut entries = Vec::with_capacity(count);
    let mut total = 0usize;
    for index in 0..count {
        let positions = coordinate(index, &axes);
        let mut graph = base.clone();
        for (axis, position) in axes.iter().zip(&positions) {
            let mut replaced = 0usize;
            let new_value = &axis.values[*position].1;
            walk(&mut graph, &mut |binding| {
                if let Binding::Literal { value, .. } = binding
                    && value.id.as_str() == axis.id
                {
                    value.value = new_value.clone();
                    replaced += 1;
                }
            });
            if replaced != 1 {
                return Err(config("Candidate literal identity changed"));
            }
        }
        let bytes = serde_json::to_vec(&graph)
            .map_err(|e| SaphoError::new(ErrorCode::Config, e.to_string()))?;
        if bytes.len() > CANDIDATE_BYTES {
            return Err(limit("Candidate exceeds one MiB"));
        }
        total = total
            .checked_add(bytes.len())
            .filter(|n| *n <= TOTAL_BYTES)
            .ok_or_else(|| limit("Sweep output exceeds 32 MiB"))?;
        let serialized =
            std::str::from_utf8(&bytes).map_err(|_| config("Candidate JSON is not UTF-8"))?;
        let parsed = GraphSpec::parse_with_format(serialized, GraphFormat::Json)?;
        if parsed != graph {
            return Err(config("Candidate JSON round trip changed graph"));
        }
        crate::inspect(&parsed, &PrimitiveRegistry::default())?;
        let file = format!("candidate-{index:04}.json");
        let entry = ManifestEntry {
            index,
            file: file.clone(),
            coordinates: axes
                .iter()
                .zip(&positions)
                .map(|(axis, position)| Coordinate {
                    id: &axis.id,
                    value: &axis.values[*position].0,
                })
                .collect(),
            sha256: sha256(&bytes),
        };
        files.push((file, bytes));
        entries.push(entry);
    }
    let manifest = serde_json::to_vec(&Manifest {
        format: "sapho-sweep-v1",
        base_sha256: sha256(base_bytes),
        grid_sha256: sha256(grid_bytes),
        entries,
    })
    .map_err(|e| SaphoError::new(ErrorCode::Config, e.to_string()))?;
    total
        .checked_add(manifest.len())
        .filter(|n| *n <= TOTAL_BYTES)
        .ok_or_else(|| limit("Sweep output exceeds 32 MiB"))?;
    Ok(Plan { files, manifest })
}
fn publish_with_hook(
    plan: &Plan,
    directory: &Path,
    mut hook: impl FnMut(usize) -> Result<(), CliError>,
) -> Result<(), CliError> {
    fs::create_dir(directory).map_err(|error| io(directory, error))?;
    let mut created = Vec::with_capacity(plan.files.len() + 1);
    let result = (|| {
        let outputs = plan
            .files
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
            .chain(std::iter::once((
                "sweep-manifest.json",
                plan.manifest.as_slice(),
            )));
        for (index, (name, bytes)) in outputs.enumerate() {
            let path = directory.join(name);
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|error| io(&path, error))?;
            created.push(path.clone());
            file.write_all(bytes)
                .and_then(|()| file.sync_all())
                .map_err(|error| io(&path, error))?;
            hook(index)?;
        }
        Ok(())
    })();
    if result.is_err() {
        for path in created.into_iter().rev() {
            let _ = fs::remove_file(path);
        }
        let _ = fs::remove_dir(directory);
    }
    result
}
/// Generate and publish every validated literal-grid candidate without graph evaluation.
pub fn sweep(
    base_path: &Path,
    grid_path: &Path,
    output_dir: &Path,
    maximum: usize,
) -> Result<SweepReport, CliError> {
    let base_bytes = read_bytes(base_path, INPUT_BYTES)?;
    let grid_bytes = read_bytes(grid_path, INPUT_BYTES)?;
    let format = select_format(base_path, None)?;
    let source =
        std::str::from_utf8(&base_bytes).map_err(|_| config("Base graph must be UTF-8"))?;
    let graph = GraphSpec::parse_with_format(source, format)?;
    let plan = plan(&graph, &base_bytes, &grid_bytes, maximum)?;
    publish_with_hook(&plan, output_dir, |_| Ok(()))?;
    Ok(SweepReport {
        output_dir: output_dir.into(),
        candidates: plan.files.len(),
        manifest: output_dir.join("sweep-manifest.json"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sapho_core::{Datum, NodeId, SourceId, SourceRef};
    use sapho_graph::{NodeSpec, Operation};

    fn base() -> GraphSpec {
        let mut weights = Datum::new(
            "weights",
            Value::List(vec![
                Datum::new("left", Value::Number(1.0)).unwrap(),
                Datum::new("right", Value::Number(2.0)).unwrap(),
            ]),
        )
        .unwrap();
        weights.sources.push(SourceRef {
            source: SourceId::new("fixture").unwrap(),
            start: Some(0),
            end: Some(2),
        });
        GraphSpec {
            inputs: BTreeMap::new(),
            nodes: vec![NodeSpec {
                id: NodeId::new("decision").unwrap(),
                operation: Operation::Record {},
                inputs: BTreeMap::from([
                    (
                        "cutoff".into(),
                        Binding::Literal {
                            value: Datum::new(
                                "cutoff",
                                Value::Probability(Probability::new(0.5).unwrap()),
                            )
                            .unwrap(),
                            value_type: ValueType::Probability,
                        },
                    ),
                    (
                        "weights".into(),
                        Binding::Literal {
                            value: weights,
                            value_type: ValueType::list(ValueType::Number),
                        },
                    ),
                ]),
                guard: None,
            }],
            outputs: BTreeMap::from([(
                "result".into(),
                Binding::Node {
                    node: NodeId::new("decision").unwrap(),
                    port: "result".into(),
                    path: vec![],
                },
            )]),
            subgraphs: BTreeMap::new(),
        }
    }
    fn grid(bytes: &str) -> Vec<u8> {
        bytes.as_bytes().to_vec()
    }
    fn value_at(graph: &GraphSpec, name: &str) -> Value {
        let Binding::Literal { value, .. } = &graph.nodes[0].inputs[name] else {
            panic!("literal")
        };
        value.value.clone()
    }

    /// Trace: FR-079-AC-1, FR-079-AC-2, FR-080-AC-1, IT-016-SC-01, IT-016-SC-02
    #[test]
    fn product_order_metadata_and_manifest_bytes_are_deterministic() {
        let base = base();
        let base_bytes = serde_json::to_vec(&base).unwrap();
        let three = grid(r#"{"axes":[{"id":"cutoff","values":[0.6,0.7,0.8]}]}"#);
        let simple = plan(&base, &base_bytes, &three, 16).unwrap();
        assert_eq!(
            simple
                .files
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            [
                "candidate-0000.json",
                "candidate-0001.json",
                "candidate-0002.json"
            ]
        );
        for (index, (_, bytes)) in simple.files.iter().enumerate() {
            let candidate = GraphSpec::parse_with_format(
                std::str::from_utf8(bytes).unwrap(),
                GraphFormat::Json,
            )
            .unwrap();
            assert_eq!(
                value_at(&candidate, "cutoff"),
                Value::Probability(Probability::new([0.6, 0.7, 0.8][index]).unwrap())
            );
            let mut expected = base.clone();
            let Binding::Literal { value, .. } =
                expected.nodes[0].inputs.get_mut("cutoff").unwrap()
            else {
                panic!("literal")
            };
            value.value = value_at(&candidate, "cutoff");
            assert_eq!(candidate, expected);
        }
        let two_axes = grid(
            r#"{"axes":[{"id":"cutoff","values":[0.6,0.7,0.8]},{"id":"weights","values":[[0.25,0.75],[0.5,0.5]]}]}"#,
        );
        let a = plan(&base, &base_bytes, &two_axes, 16).unwrap();
        let b = plan(&base, &base_bytes, &two_axes, 16).unwrap();
        assert_eq!(a.files, b.files);
        assert_eq!(a.manifest, b.manifest);
        assert_eq!(a.files.len(), 6);
        let manifest: JsonValue = serde_json::from_slice(&a.manifest).unwrap();
        assert_eq!(manifest["format"], "sapho-sweep-v1");
        assert_eq!(manifest["base_sha256"], sha256(&base_bytes));
        assert_eq!(manifest["grid_sha256"], sha256(&two_axes));
        for (index, (name, bytes)) in a.files.iter().enumerate() {
            let candidate = GraphSpec::parse_with_format(
                std::str::from_utf8(bytes).unwrap(),
                GraphFormat::Json,
            )
            .unwrap();
            assert_eq!(
                value_at(&candidate, "cutoff"),
                Value::Probability(Probability::new([0.6, 0.7, 0.8][index / 2]).unwrap())
            );
            let Value::List(items) = value_at(&candidate, "weights") else {
                panic!("weights")
            };
            assert_eq!(
                items.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
                ["left", "right"]
            );
            assert_eq!(
                items.iter().map(|d| &d.value).collect::<Vec<_>>(),
                if index % 2 == 0 {
                    vec![&Value::Number(0.25), &Value::Number(0.75)]
                } else {
                    vec![&Value::Number(0.5), &Value::Number(0.5)]
                }
            );
            let Binding::Literal { value, .. } = &candidate.nodes[0].inputs["weights"] else {
                panic!("literal")
            };
            let Binding::Literal {
                value: original, ..
            } = &base.nodes[0].inputs["weights"]
            else {
                panic!("literal")
            };
            assert_eq!(value.sources, original.sources);
            assert_eq!(manifest["entries"][index]["index"], index);
            assert_eq!(manifest["entries"][index]["file"], name.as_str());
            assert_eq!(manifest["entries"][index]["sha256"], sha256(bytes));
            assert_eq!(
                manifest["entries"][index]["coordinates"][0]["value"],
                [0.6, 0.7, 0.8][index / 2]
            );
            assert_eq!(
                manifest["entries"][index]["coordinates"][1]["value"],
                if index % 2 == 0 {
                    serde_json::json!([0.25, 0.75])
                } else {
                    serde_json::json!([0.5, 0.5])
                }
            );
        }
    }

    /// Trace: FR-079-AC-3, FR-079-AC-4, FR-079-AC-5, IT-016-SC-03
    #[test]
    fn invalid_grid_refuses_before_publication_and_ceiling_is_independent() {
        let base = base();
        let bytes = serde_json::to_vec(&base).unwrap();
        for bad in [
            r#"{"axes":[{"id":"cutoff","values":[0.6,1.2]}]}"#,
            r#"{"axes":[{"id":"weights","values":[[0.2,"bad"]]}]}"#,
            r#"{"axes":[{"id":"weights","values":[[0.2]]}]}"#,
            r#"{"axes":[{"id":"missing","values":[0.6]}]}"#,
            r#"{"axes":[{"id":"cutoff","values":[0.6]},{"id":"cutoff","values":[0.7]}]}"#,
            r#"{"axes":[]}"#,
            r#"{"axes":[{"id":"cutoff","values":[] }]}"#,
            r#"{"axes":[{"id":"cutoff","values":[0.6],"unknown":1}]}"#,
        ] {
            assert!(plan(&base, &bytes, &grid(bad), 16).is_err(), "{bad}");
        }
        let mut ambiguous = base.clone();
        ambiguous
            .outputs
            .insert("extra".into(), ambiguous.nodes[0].inputs["cutoff"].clone());
        assert!(
            plan(
                &ambiguous,
                &bytes,
                &grid(r#"{"axes":[{"id":"cutoff","values":[0.6]}]}"#),
                16
            )
            .is_err()
        );
        let list = |n: usize| (0..n).map(|_| "0.6").collect::<Vec<_>>().join(",");
        let weights = |n: usize| (0..n).map(|_| "[0.25,0.75]").collect::<Vec<_>>().join(",");
        let product_grid = |a: usize, b: usize| {
            format!(
                r#"{{"axes":[{{"id":"cutoff","values":[{}]}},{{"id":"weights","values":[{}]}}]}}"#,
                list(a),
                weights(b)
            )
        };
        assert_eq!(
            plan(&base, &bytes, product_grid(4, 4).as_bytes(), 16)
                .unwrap()
                .files
                .len(),
            16
        );
        assert!(plan(&base, &bytes, product_grid(5, 4).as_bytes(), 16).is_err());
        assert_eq!(
            plan(&base, &bytes, product_grid(5, 4).as_bytes(), 20)
                .unwrap()
                .files
                .len(),
            20
        );
        assert!(plan(&base, &bytes, product_grid(5, 4).as_bytes(), 257).is_err());
        assert!(plan(&base, &bytes, product_grid(5, 4).as_bytes(), 19).is_err());
        let too_many_values = format!(r#"{{"axes":[{{"id":"cutoff","values":[{}]}}]}}"#, list(17));
        assert!(plan(&base, &bytes, too_many_values.as_bytes(), 256).is_err());
        let too_many_axes = format!(
            r#"{{"axes":[{}]}}"#,
            (0..9)
                .map(|index| format!(r#"{{"id":"axis-{index}","values":[0.6]}}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert!(plan(&base, &bytes, too_many_axes.as_bytes(), 256).is_err());
        let mut unsupported = base.clone();
        let Binding::Literal { value, value_type } =
            unsupported.nodes[0].inputs.get_mut("cutoff").unwrap()
        else {
            panic!("literal")
        };
        value.value = Value::Text("prompt".into());
        *value_type = ValueType::Text;
        assert!(
            plan(
                &unsupported,
                &bytes,
                &grid(r#"{"axes":[{"id":"cutoff","values":[0.6]}]}"#),
                16
            )
            .is_err()
        );
    }

    /// Trace: FR-080-AC-2, FR-080-AC-5, IT-016-SC-04
    #[test]
    fn publisher_refuses_collisions_and_cleans_only_created_files() {
        let base = base();
        let bytes = serde_json::to_vec(&base).unwrap();
        let plan = plan(
            &base,
            &bytes,
            &grid(r#"{"axes":[{"id":"cutoff","values":[0.6,0.7]}]}"#),
            16,
        )
        .unwrap();
        let root = tempfile::tempdir().unwrap();
        let existing_file = root.path().join("existing-file");
        fs::write(&existing_file, b"sentinel").unwrap();
        assert!(publish_with_hook(&plan, &existing_file, |_| Ok(())).is_err());
        assert_eq!(fs::read(&existing_file).unwrap(), b"sentinel");
        let existing_dir = root.path().join("existing-dir");
        fs::create_dir(&existing_dir).unwrap();
        assert!(publish_with_hook(&plan, &existing_dir, |_| Ok(())).is_err());
        assert!(fs::read_dir(&existing_dir).unwrap().next().is_none());
        let failed = root.path().join("failed");
        assert!(
            publish_with_hook(&plan, &failed, |index| {
                if index == 0 {
                    return Err(config("Injected write failure"));
                }
                Ok(())
            })
            .is_err()
        );
        assert!(!failed.exists());
        assert_eq!(fs::read(&existing_file).unwrap(), b"sentinel");
    }
}
