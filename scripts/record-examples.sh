#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Agent-IX
#
# Re-record the Jev answers behind the documentation examples. Makes one live
# request per code-review case and two or three per requirement statement.
# Needs a CLI built with --features jev and TYPESAFE_API_KEY. Jev's answers
# vary by a few hundredths between runs, so a new recording changes the
# documented numbers: update the docs and scripts/check_docs.py (which pins
# them), and re-render the figures (docs/images/figures.py reads the
# recordings) with the docs-figures skill from agent-ix/dev-tools.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
sapho=${SAPHO:-sapho}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

bindings="$work/bindings.yaml"
cat > "$bindings" <<'YAML'
judge: {provider: jev, model: jev-1.13.0, expected_model: jev-1.13.0, distribution_policy: {kind: strict}}
expert: {provider: jev, model: jev-1.13.0, expected_model: jev-1.13.0, distribution_policy: {kind: strict}}
YAML

# Plain inputs for every code-review case, taken from the dataset.
mkdir -p "$work/cases"
python3 - "$root/examples/data/code-review-dataset.json" "$work/cases" <<'PY'
import json, sys
from pathlib import Path
for case in json.loads(Path(sys.argv[1]).read_text())["cases"]:
    plain = {name: datum["value"]["value"] for name, datum in case["inputs"].items()}
    (Path(sys.argv[2]) / f"{case['id']}.json").write_text(json.dumps(plain))
PY

record() {  # graph, output recording, input files...
  local graph=$1 out=$2; shift 2
  local parts=()
  for input in "$@"; do
    local part
    part="$work/$(basename "$out" .json)-$(basename "$input")"
    "$sapho" record "$root/examples/graphs/$graph" --input "$input" \
      --bindings "$bindings" --recording "$part" --output "$part.report"
    parts+=("$part")
  done
  python3 - "$root/examples/recordings/$out" "${parts[@]}" <<'PY'
import json, sys
from pathlib import Path
exchanges = [e for p in sys.argv[2:] for e in json.loads(Path(p).read_text())["exchanges"]]
Path(sys.argv[1]).write_text(json.dumps({"exchanges": exchanges}, indent=2) + "\n")
print(f"{sys.argv[1]}: {len(exchanges)} exchanges")
PY
}

mkdir -p "$root/examples/recordings"
record code-review.yaml code-review.json "$work"/cases/*.json
record requirement-check.yaml requirement-check.json "$root"/examples/data/requirements/*.json
