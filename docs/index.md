# Sapho documentation

Sapho evaluates typed decision graphs written in YAML or JSON. Use it from the
CLI or embed it in Rust; supply facts, model questions and your own decision
policy. Start with an offline rule, then add model calls when you need them.

| I want to… | Read |
|---|---|
| Install and run my first rule | [README quickstart](../README.md#install-the-cli) |
| Understand how a decision is made | [How Sapho works](how-it-works.md) |
| Measure and tune a rule against labelled cases | [Improve a rule](improve-a-rule.md) |
| Write and run graphs from a shell | [CLI guide](cli-guide.md) |
| Embed Sapho or register Rust code | [Rust user guide](user-guide.md) |
| Look up graph fields, ports or operations | [Graph reference](graph-reference.md) |
| Look up commands, flags, defaults or exit codes | [CLI reference](cli-reference.md) |
| Find Rust APIs, crate boundaries and errors | [Rust API reference](api-reference.md) |
| Run a complete example for a feature | [Examples and recipes](examples.md) |
| Check whether a feature has reference and examples | [Feature coverage](feature-coverage.md) |

The guides explain choices and workflows. Reference pages describe the
contracts. Runnable files supply complete programs and expected results;
`make docs-check` verifies them offline. Jev and CLM setup is documented
separately from the deterministic tutorial backend.

For adapter authors and contributors, [behavior specifications](../spec/spec.md)
define implementation obligations. They complement the user documentation.
