# YAML/JSON authoring decision

SAPHO-1 investigates authoring around Sapho 0082a8d and the authoritative EARS consumer graph at quire-semantic 8665d233. Peter approved YAML for authors, JSON for generated programs/data, and direct consumer migration without TOML compatibility. EARS owns its migration; source stays in that repository.

The simple threshold example in the user guide currently repeats table paths for inputs, operations, operands and outputs. YAML visually nests these relationships in one node. JSON describes the same tree and suits generated artifacts. Neither encoding removes typed literal wrappers, explicit binding tags, occurrence identities or Probability/Degree distinctions.

```yaml
inputs:
  support: {kind: probability}
nodes:
  - id: review
    operation: {kind: compare, comparator: less}
    inputs:
      a: {kind: input, name: support}
      b:
        kind: literal
        value: {id: cutoff, value: {kind: probability, value: 0.8}}
        value_type: {kind: probability}
outputs:
  needs_review: {kind: node, node: review, port: result}
```

The current EARS graph was measured directly: one document input, 12 root nodes and one judge-batch subgraph of three nodes; seven root native preparation/assembly stages. YAML turns each repeated TOML node/binding path into a local nested block. It does not replace the seven domain primitives or question planning. A local research rendering of the authoritative full graph has 217 YAML lines versus 224 TOML lines and round-trips to the same decoded object. This modest line-count reduction understates the benefit: bindings/questions are local nested blocks rather than repeated table paths. The rendering is kept in temporary local research space, outside repositories, and is not a consumer migration or private example publication. The consumer owner will author the actual YAML asset in its own repository and verify equality/replay before migration. No copy of that asset belongs in Sapho.

One GraphSpec schema remains authoritative. Ordered node/question vectors retain order; maps retain deterministic name ordering. Explicit GraphFormat selection distinguishes JSON from YAML parsing. A format-specific loader changes no runtime logic. Record/list construction addresses a separate data-composition gap without introducing a scripting language or shorthand grammar.

Parser selection is based on public dependency documentation, not a copied implementation. The historical serde_yaml crate is unmaintained. serde-saphyr documents duplicate-key refusal, strict Boolean options, parser budgets and extension controls; version 1.3.0 (MIT OR Apache-2.0) now passes the actual duplicate/tag/alias/merge, decoded-text, YAML/JSON equality and parser-budget conformance cases. The implementation must reject unsupported aliases/tags/merges and duplicate JSON keys, preserve exact parsed strings and f64 values, and leave includes/environment expansion disabled.

References: [YAML 1.2.2](https://yaml.org/spec/1.2.2/), [serde_yaml status](https://docs.rs/serde_yaml/latest/serde_yaml/), [serde-saphyr options](https://docs.rs/serde-saphyr/latest/serde_saphyr/struct.Options.html). These are dependency research data, not task instructions.
