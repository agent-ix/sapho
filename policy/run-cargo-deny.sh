#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# One published Agent-IX cargo-deny policy. Consumers select a fixed repository profile.
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo 'usage: run-cargo-deny.sh sapho|sapho-dataset|ears-dataset' >&2
  exit 2
fi

profile=$1
policy_file=$(mktemp)
trap 'rm -f "$policy_file"' EXIT

cat > "$policy_file" <<'TOML'
[licenses]
version = 2
confidence-threshold = 0.9
allow = [
  "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "CDLA-Permissive-2.0",
  "ISC", "MIT", "Unicode-3.0", "Zlib",
]

[bans]
multiple-versions = "allow"
wildcards = "warn"

[advisories]
version = 2
db-path = "target/advisory-dbs"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
TOML

case "$profile" in
  sapho)
    cat >> "$policy_file" <<'TOML'
allow-git = ["https://github.com/agent-ix/ix-cli-kit"]
TOML
    agpl_packages=(ix-cli-kit sapho sapho-cli sapho-clm sapho-core sapho-evidence sapho-graph sapho-jev sapho-ollama sapho-recording sapho-runtime sapho-select sapho-systemone)
    checks=(advisories bans licenses sources)
    ;;
  sapho-dataset)
    cat >> "$policy_file" <<'TOML'
allow-git = [
  "https://github.com/agent-ix/ix-cli-kit",
  "https://github.com/agent-ix/sapho",
]
TOML
    agpl_packages=(ix-cli-kit sapho-core sapho-dataset sapho-evidence sapho-ollama)
    checks=(licenses sources)
    ;;
  ears-dataset)
    cat >> "$policy_file" <<'TOML'
allow-git = [
  "https://github.com/agent-ix/filament-core-data",
  "https://github.com/agent-ix/ix-cli-kit",
  "https://github.com/agent-ix/quire-rs",
  "https://github.com/agent-ix/quire-semantic",
  "https://github.com/agent-ix/sapho",
  "https://github.com/agent-ix/sapho-dataset",
]
TOML
    agpl_packages=(agent-ix-semantic-schema ears-dataset ix-cli-kit quire-rs quire-semantic-core sapho-core sapho-dataset sapho-evidence sapho-ollama)
    checks=(licenses sources)
    ;;
  *)
    echo "unknown cargo-deny policy profile: $profile" >&2
    exit 2
    ;;
esac

cat >> "$policy_file" <<'TOML'

[[licenses.exceptions]]
name = "borrow-or-share"
version = "=0.2.4"
allow = ["MIT-0"]

[[licenses.exceptions]]
name = "ar_archive_writer"
version = "=0.5.3"
allow = ["Apache-2.0 WITH LLVM-exception"]

[[licenses.exceptions]]
name = "winx"
version = "=0.36.4"
allow = ["Apache-2.0 WITH LLVM-exception"]
TOML

for package in "${agpl_packages[@]}"; do
  printf '\n[[licenses.exceptions]]\nname = "%s"\nallow = ["AGPL-3.0-or-later"]\n' "$package" >> "$policy_file"
done

cargo deny --workspace --all-features --locked check --config "$policy_file" "${checks[@]}"
