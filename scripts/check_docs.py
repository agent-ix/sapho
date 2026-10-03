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
        for target in re.findall(r"!?\[[^\]]*\]\(([^\s)]+)(?:\s+[^)]*)?\)", content):
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
    selected = json.loads(invoke(["select", "files", "--root", "examples/graphs", "--include", "**/*.yaml", "--exclude", "**/review-conservative.yaml"]))
    assert len(selected["items"]["value"]["value"]) == 4
    file_run = json.loads(invoke(["run", "examples/graphs/files.yaml", "--typed-input"], data=json.dumps(selected)))
    assert file_run["outputs"]
    selected = json.loads(invoke(["select", "json", "--input", "examples/data/selection.json", "--pointer", "/records", "--schema", "examples/data/selection-schema.json"]))
    assert len(selected["items"]["value"]["value"]) == 2
    invoke(["select", "json", "--input", "examples/data/selection.json", "--pointer", "/missing", "--schema", "examples/data/selection-schema.json"], code=2)
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
