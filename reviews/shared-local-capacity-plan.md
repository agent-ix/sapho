# Shared local model capacity implementation plan

Current evidence: `OllamaBackend` owns one instance-local Tokio semaphore; separate
instances and processes do not share it. This contradicts the EARS architecture's
explicit competing-worker capacity requirement. FR-048 now specifies the missing
reusable provider boundary rather than placing resource logic in EARS.

Implement a documented `new_shared` constructor with an explicit stable lock path.
Prepare a no-follow regular file outside Tokio; use OS nonblocking exclusive file
locks, asynchronously polling within the existing timeout. Each backend owns a separately opened file handle; its existing semaphore
serializes calls on that handle. Independent backends never clone a shared descriptor. The lease guard releases on drop.
Keep existing constructor semantics explicit for library users; campaign CLIs
require the shared path for live operation. Expose a CLI flag in generic Sapho and
EARS, retain resource configuration in provenance, and update commands/examples.

Verify with actual subprocess lease owners, bounded loopback dispatch observation,
waiter cancellation and owner termination. Existing transport/capture/replay tests
must remain unchanged. Build one Cargo process at a time in Draco's shared target.
Review lock portability, cancellation, stable inode ownership, timeout denominator
and no-dispatch failures before implementation. No implementation or acceptance
claim is made by this plan.

Provider implementation verified: 8 tests passed including a real subprocess
lease owner terminated by the parent, cancelled waiter exclusion and no-follow
path refusal. Strict provider Clippy passed. Generic CLI wiring is implemented;
consumer integration and concurrent HTTP dispatch tests remain pending.
