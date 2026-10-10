#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Agent-IX
"""Check local documentation links and run the offline CLI recipes."""
import json
import math
import os
from pathlib import Path
import re
import subprocess
import tempfile
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / "target" / "debug" / ("sapho.exe" if os.name == "nt" else "sapho")


def invoke(args, *, data=None, code=0):
    completed = subprocess.run([str(BINARY), *map(str, args)], input=data,
                               text=True, capture_output=True, cwd=ROOT, timeout=30)
    if completed.returncode != code:
        raise AssertionError(f"sapho {' '.join(map(str, args))}: expected exit {code}, "
                             f"got {completed.returncode}\n{completed.stdout}\n{completed.stderr}")
    return completed.stdout


def anchors(path):
    result = set()
    counts = {}
    in_fence = False
    for line in path.read_text().splitlines():
        if line.startswith("```"):
            in_fence = not in_fence
        if in_fence:
            continue
        match = re.match(r"^#{1,6}\s+(.+?)\s*#*\s*$", line)
        if match:
            slug = re.sub(r"[^\w\- ]", "", match[1].lower()).replace(" ", "-")
            count = counts.get(slug, 0)
            counts[slug] = count + 1
            result.add(slug if count == 0 else f"{slug}-{count}")
    return result


def links():
    paths = [ROOT / "README.md", *sorted((ROOT / "docs").glob("*.md"))]
    for path in paths:
        # Remove fenced code before inspecting Markdown links.
        content = re.sub(r"```[^\n]*\n.*?```", "", path.read_text(), flags=re.S)
        markdown = re.findall(r"!?\[[^\]]*\]\(([^\s)]+)(?:\s+[^)]*)?\)", content)
        html = re.findall(r'\b(?:src|srcset)="([^"]+)"', content)
        for target in markdown + html:
            target = target.strip("<>")
            parsed = urlsplit(target)
            if parsed.scheme or parsed.netloc:
                continue
            destination = (path.parent / unquote(parsed.path)).resolve() if parsed.path else path
            if not destination.exists():
                raise AssertionError(f"Broken link in {path.relative_to(ROOT)}: {target}")
            if parsed.fragment and destination.suffix == ".md":
                if unquote(parsed.fragment) not in anchors(destination):
                    raise AssertionError(f"Broken heading in {path.relative_to(ROOT)}: {target}")


def plain(value):
    kind = value["kind"]
    payload = value.get("value")
    if kind == "list":
        return [plain(d["value"]) for d in payload]
    if kind == "record":
        return {k: plain(v) for k, v in payload.items()}
    if kind == "optional":
        return None if payload is None else plain(payload)
    return payload


def equal(actual, expected):
    if isinstance(expected, bool) or expected is None:
        assert actual is expected, (actual, expected)
    elif isinstance(expected, (int, float)):
        assert math.isclose(actual, expected, abs_tol=1e-12), (actual, expected)
    elif isinstance(expected, dict):
        assert actual.keys() == expected.keys(), (actual, expected)
        for key, value in expected.items():
            equal(actual[key], value)
    elif isinstance(expected, list):
        assert len(actual) == len(expected), (actual, expected)
        for a, b in zip(actual, expected):
            equal(a, b)
    else:
        assert actual == expected, (actual, expected)


def recorded():
    """The README, how-it-works and improve-a-rule numbers, replayed from the recorded Jev answers."""
    review = ["--input", "examples/data/code-review-input.json", "--recording", "examples/recordings/code-review.json"]
    for graph, needs_review in [("code-review", True), ("code-review-relaxed", False)]:
        report = json.loads(invoke(["replay", f"examples/graphs/{graph}.yaml", *review]))
        equal({k: plain(v["value"]) for k, v in report["outputs"].items()},
              {"contract_risk": 0.79, "test_gap": 0.61, "needs_review": needs_review})
    ask = next(n for n in report["trace"]["nodes"] if n["path"] == ["root", "ask"])
    assert ask["model"]["response"]["usage"] == {"input_tokens": 424, "output_tokens": 39}
    assert len(json.loads((ROOT / "examples/recordings/code-review.json").read_text())["exchanges"]) == 8
    splits = [c["split"] for c in json.loads((ROOT / "examples/data/code-review-dataset.json").read_text())["cases"]]
    assert (splits.count("development"), splits.count("held_out")) == (6, 2)
    stages = json.loads(invoke(["inspect", "examples/graphs/code-review.yaml"]))["groups"][0]["stages"]
    assert stages == [["context", "questions"], ["ask"], ["contract_risk", "test_gap"],
                      ["risky_contract", "untested"], ["either"], ["needs_review"]], stages
    dataset = ["--dataset", "examples/data/code-review-dataset.json", "--replay", "examples/recordings/code-review.json"]
    for split, confusion in [("development", (3, 3)), ("held_out", (1, 1))]:
        measured = json.loads(invoke(["measure", "examples/graphs/code-review.yaml", *dataset, "--split", split]))
        metrics = measured["measurement"]["outputs"]["needs_review"]["metrics"]
        assert metrics["agreement"] == 1.0, metrics
        assert (metrics["confusion"]["true_positive"], metrics["confusion"]["true_negative"]) == confusion, metrics
    held_out = {run["id"]: run["report"]["outputs"] for run in measured["runs"]}
    equal({k: plain(v["value"]) for k, v in held_out["error-variant"].items()},
          {"contract_risk": 0.48, "test_gap": 0.82, "needs_review": True})
    tuned = json.loads(invoke(["tune", "--candidate", "examples/graphs/code-review.yaml",
                               "--candidate", "examples/graphs/code-review-relaxed.yaml", *dataset,
                               "--output-name", "needs_review", "--metric", "agreement"]))
    assert [r["input_index"] for r in tuned["ranking"]] == [0, 1]
    assert [round(r["score"], 2) for r in tuned["ranking"]] == [1.0, 0.67]
    relaxed = json.loads(invoke(["measure", "examples/graphs/code-review-relaxed.yaml", *dataset, "--split", "development"]))
    missed = sorted(p["case"] for p in relaxed["measurement"]["predictions"] if p["predicted"]["value"] != p["label"])
    assert missed == ["csv-export", "default-timeout"], missed
    csv = next(run["report"]["outputs"] for run in relaxed["runs"] if run["id"] == "csv-export")
    assert plain(csv["test_gap"]["value"]) == 0.83
    run_help = invoke(["run", "--help"])
    assert re.search(r"--max-model-calls <MAX_MODEL_CALLS>\s+\[default: 128\]", run_help)
    assert re.search(r"--timeout-secs <TIMEOUT_SECS>\s+\[default: 60\]", run_help)
    requirements = ["--recording", "examples/recordings/requirement-check.json"]
    expected = {
        "report": ({"asked_expert": True, "first_answer": 0.78, "complete": 0.6, "conditional": 1.0,
                    "testable": 0.48, "ok": False}, "completed"),
        "brake-lamp": ({"asked_expert": False, "first_answer": 0.95, "complete": 0.95, "conditional": 1.0,
                        "testable": 0.99, "ok": True}, "skipped"),
        "cabin-light": ({"asked_expert": False, "first_answer": 0.96, "complete": 0.96, "conditional": 1.0,
                         "testable": 0.99, "ok": True}, "skipped"),
        "pump": ({"asked_expert": False, "first_answer": 0.84, "complete": 0.84, "conditional": 1.0,
                  "testable": 0.98, "ok": True}, "skipped"),
        "fast": ({"asked_expert": False, "first_answer": 0.16, "complete": 0.16, "conditional": 0.61,
                  "testable": 0.36, "ok": False}, "skipped"),
    }
    for name, (outputs, expert) in expected.items():
        report = json.loads(invoke(["replay", "examples/graphs/requirement-check.yaml",
                                    "--input", f"examples/data/requirements/{name}.json", *requirements]))
        equal({k: plain(v["value"]) for k, v in report["outputs"].items()}, outputs)
        statuses = {node["path"][-1]: node["status"] for node in report["trace"]["nodes"]}
        assert statuses["expert"] == expert, (name, statuses["expert"])
        layer1 = next(n for n in report["trace"]["nodes"] if n["path"][-1] == "layer1")
        answers = layer1["model"]["response"]["answers"]
        if name == "report":
            assert answers["kind"]["probabilities"]["event"] == 1.0
        if name == "fast":
            assert answers["kind"]["probabilities"] == {"event": 0.61, "state": 0.0, "ubiquitous": 0.39, "unwanted": 0.0}
            assert (answers["kind"]["selected"], answers["kind"]["confidence"]) == ("event", 0.48)
            assert answers["testable"]["probabilities"] == {"0": 0.01, "1": 0.63, "2": 0.36}
            assert (answers["testable"]["expected"], answers["testable"]["confidence"]) == (1.35, 0.45)


def recipes():
    cases = json.loads((ROOT / "examples/reference/cases.json").read_text())
    for case in cases:
        if case.get("backend") or case.get("native"):
            continue  # These are executed by the public Rust host.
        for extension in ("yaml", "json"):
            graph = f"examples/reference/{case['name']}.{extension}"
            invoke(["validate", graph])
            inspection = json.loads(invoke(["inspect", graph]))
            assert set(inspection["signature"]["outputs"]) == set(case["expected"])
            report = json.loads(invoke(["run", graph, "--input", f"examples/reference/{case['input']}"]))
            equal({k: plain(v["value"]) for k, v in report["outputs"].items()}, case["expected"])
    for support, expected_exit in [(0.7, 1), (0.8, 0), (0.9, 0)]:
        result = json.loads(invoke(["run", "examples/graphs/review.yaml", "--fail-on", "needs_review"], data=json.dumps({"support": support}), code=expected_exit))
        assert plain(result["outputs"]["needs_review"]["value"]) is (expected_exit == 1)
    limited = json.loads(invoke(["run", "examples/reference/collections.yaml", "--input", "examples/reference/collections-input.json", "--max-items", "1"], code=2))
    assert limited["error"]["code"] == "limit_exceeded"
    assert limited["trace"]["nodes"]
    selected = json.loads(invoke(["select", "files", "--root", "examples/graphs", "--include", "**/*.yaml", "--exclude", "**/review-conservative.yaml", "--exclude", "**/router.yaml", "--exclude", "**/escalation.yaml"]))
    assert len(selected["items"]["value"]["value"]) == 7
    file_run = json.loads(invoke(["run", "examples/graphs/files.yaml", "--typed-input"], data=json.dumps(selected)))
    assert file_run["outputs"]
    selected = json.loads(invoke(["select", "json", "--input", "examples/data/selection.json", "--pointer", "/records", "--schema", "examples/data/selection-schema.json"]))
    assert len(selected["items"]["value"]["value"]) == 2
    invoke(["select", "json", "--input", "examples/data/selection.json", "--pointer", "/missing", "--schema", "examples/data/selection-schema.json"], code=2)
    recorded()
    with tempfile.TemporaryDirectory(prefix="sapho-docs-") as temp:
        directory = Path(temp)
        recording = directory / "recording.json"
        trace = directory / "trace.json"
        report = directory / "report.json"
        invoke(["record", "examples/graphs/review.yaml", "--recording", recording, "--trace", trace, "--output", report], data='{"support":0.7}')
        assert json.loads(recording.read_text()) == {"exchanges": []}
        assert json.loads(trace.read_text())["nodes"]
        replay = json.loads(invoke(["replay", "examples/graphs/review.yaml", "--recording", recording], data='{"support":0.7}'))
        assert replay["outputs"] == json.loads(report.read_text())["outputs"]
        invoke(["record", "examples/graphs/review.yaml", "--recording", recording], data='{"support":0.7}', code=2)
        data = "examples/data/review-dataset.json"
        for split, selected_count in [("development", 2), ("held_out", 1)]:
            measured = json.loads(invoke(["measure", "examples/graphs/review.yaml", "--dataset", data, "--split", split]))
            measurement = measured["measurement"]
            assert measurement["selected_cases"] == selected_count
            assert measurement["outputs"]["needs_review"]["metrics"]["agreement"] == 1.0
        tuned = json.loads(invoke(["tune", "--candidate", "examples/graphs/review.yaml", "--candidate", "examples/graphs/review-conservative.yaml", "--dataset", data, "--output-name", "needs_review", "--metric", "agreement"]))
        assert [r["input_index"] for r in tuned["ranking"]] == [0, 1]
        assert [r["score"] for r in tuned["ranking"]] == [1.0, 0.5]
        export = directory / "development.jsonl"
        invoke(["export-training", "--dataset", data, "--output", export])
        rows = [json.loads(line) for line in export.read_text().splitlines()]
        assert [r["id"] for r in rows] == ["clear", "review"]
        if os.name != "nt":
            # All writes/commits are confined to this disposable tutorial fixture.
            git_root = directory / "git"
            git_root.mkdir()
            def git(*args):
                return subprocess.run(["git", "-c", "core.hooksPath=/dev/null", "-c", "core.fsmonitor=false", "-c", "user.name=Sapho Tutorial", "-c", "user.email=tutorial@example.invalid", *args], cwd=git_root, check=True, capture_output=True, timeout=30)
            git("init", "-q")
            (git_root / "example.txt").write_text("before\n")
            git("add", "example.txt")
            git("-c", "commit.gpgsign=false", "commit", "-qm", "Tutorial fixture")
            (git_root / "example.txt").write_text("after\n")
            for mode, count in [("working_tree", 1), ("staged", 0), ("revisions", 0)]:
                args = ["select", "git", "--root", git_root, "--mode", mode]
                if mode == "revisions":
                    args += ["--base", "HEAD", "--head", "HEAD"]
                patches = json.loads(invoke(args))
                assert len(patches["items"]["value"]["value"]) == count
            git("add", "example.txt")
            patches = json.loads(invoke(["select", "git", "--root", git_root, "--mode", "staged"]))
            assert len(patches["items"]["value"]["value"]) == 1


def main():
    links()
    recipes()
    print("Documentation links and offline CLI recipes verified")

if __name__ == "__main__":
    main()
