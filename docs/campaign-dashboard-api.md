# Campaign dashboard JSON projection

Status: proposed interface, implementation and verification pending. This is a
machine interface for read-only integrations, separate from Ratatui display text.
The CLI command is `sapho campaign --state PRIVATE_STATE dashboard`, with optional
`--attempt POSITIVE_ID`. Domain hosts expose an equivalent `dashboard` command.
No model dispatch, writer lock, controls or publication occurs during projection.

## Version 1 contract

The response is an object with these fields:

| Field | Meaning |
|---|---|
| `schema` | Integer 1. Unknown versions are refused by consumers. |
| `generated_unix_ms` | Projection generation time, not the last durable update. |
| `campaign` | Durable sequence, control revision, paused, job count and exhaustive attempt-state counts. |
| `metrics` | Bounded map of typed metric records. |
| `complete` | Domain completion predicate; null when the host supplies none. |
| `live` | Timestamped provider activity observation, or null when unavailable, stale or idle/unknown. |
| `selected_attempt` | One structured retained attempt, or null when no selection was requested. |

Metric records contain `value`, optional `target`, `unit`, `meaning` and
`authority`. Units are explicit (`jobs`, `attempts`, `passages`, `repositories`,
`domains`, `readings`, `packets`, `judgments`, `http_dispatches`). Meaning is a
stable machine category rather than a translated label. Authority identifies
durable execution counts, downstream domain assessment, supplied human response,
or ephemeral provider activity. A rendered label is optional presentation only.
Ratios do not imply correctness or completion; the host's explicit `complete`
predicate is authoritative. Domain hosts own the meaning of their named metrics.

Selected attempts contain the real ID, stage, state, optional parent ID and
immutable request/response SHA256 references. They exclude operator retry reason,
source text, private error strings, prompts, raw model output and credentials.
No invented attempt IDs or inferred last-success output are permitted. An unknown
requested attempt is refused rather than returning the nearest attempt.
Provenance is expressed through the retained immutable request hash and response
hash. Arbitrary backend metadata is not copied into the default feed.

Provider observations carry their observed timestamp, explicit maximum age,
attempt ID, model identity, dispatch-attempt count, receipt count and in-flight
flag. The host validates association with the current dispatching attempt and
rejects future timestamps. Absence means unavailable/idle/unknown, never zero
latency, zero token usage or successful completion. Dispatches measure activity,
not labelled correctness, novelty or usefulness.

## Bounds and consistency

At most 256 metric records, 128 UTF8 bytes per metric key, 256 bytes per optional
label and 256 bytes per selected stage are accepted. The serialized response is
bounded to 256 KiB and refuses oversize rather than truncating fields. The command
does not enumerate all attempts or read all output blobs. Selection reads one
exact retained attempt. Durable counts, metrics and selection come from one
SQLite read transaction and name the same event sequence. Ephemeral provider
observations are explicitly outside that durable transaction.

Source/native interpretation inspection is a separate explicit downstream
operation with its own structured schema, source disclosure option and bounds.
It is not emitted by polling the default dashboard. Generic Sapho has no EARS,
Quire, novelty, independent-reading or human-packet interpretation dependencies.
Mods call the host CLI and never parse TUI text or open the campaign database.

## Verification required

Verify schema version/strict decoding, bounded collections and serialization,
selection identity/parent/hash fidelity, unknown-selection refusal, unchanged
event sequence, snapshot consistency with a live writer, source-free default
output, explicit null completion/telemetry, stale/future observation rejection
and standalone CLI JSON decoding. These are interface tests, not model-quality
or data-completion evidence. Send the verified revision and executable receipt
to consumers only after these checks pass.
