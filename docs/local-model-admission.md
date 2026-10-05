# Local-model request admission

A host calls `sapho::ollama::RequestBudget::measure(&request, &limits)` before
provider I/O, then checks `allowed()`. This measures the complete typed request,
the chosen prompt encoding and the exact HTTP body including output schema and
options. It returns only a typed request hash and numeric component/cap facts;
no source, questions, prompt or response is included in the budget object.
A returned budget can be over its context or wire cap. An unmeasurable/invalid
request returns the owning typed error. No service inspection, capacity file,
HTTP, generation, shortening or implicit retry occurs during measurement.

The default `model_request_json` prompt remains unchanged. A host may explicitly
choose `framed_state_v1` as a new recipe: complete text fields are length-framed
UTF8 bodies, and nested typed values, metadata and full questions remain typed
JSON. A text field that happens to contain JSON remains text. This avoids another
layer of escaping while keeping each field's complete bytes. It does not remove
state dependencies or establish native/domain coverage.

Actual dispatch uses the same prompt and wire encoders. Prompt components are
bytes for one call, not aggregate tokens. Provider receipts preserve the exact
chosen format, typed request hash, raw wire references, elapsed time, HTTP send
state and available service usage. The generic recording session additionally
retains typed backend request/response bodies and source-free observations for
all configured providers. A backend invocation does not establish HTTP dispatch;
a typed return does not establish valid answers. Cancellation is unknown transport
outcome. Response retention failure remains a failure with known usage retained;
missing retained response is unavailable, never a fabricated successful proposal.

These APIs admit one complete request. A whole-classification host must first
bound every answer-dependent future request family and state dependency shape,
including possible grammar, actions and attachments, and use this encoder for
those conservative envelopes. It must also account for total request/capture
bounds. A successful scripted dry-run describes one answer path and cannot prove
that bound. Neither this API nor framing clears the current EARS inference hold.

Keep raw receipts in private host storage. Default dashboard consumers receive
only the bounded structured snapshot in [Dashboard API](campaign-dashboard-api.md),
not raw sources, provider bodies or SQLite access. No EARS data/policy is owned here.
