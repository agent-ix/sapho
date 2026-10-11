---
type: master-requirements
name: sapho-cli
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

Checked command-line invocation and host-owned I/O, backends and exit policy.

## Scope

This module owns only the requirements indexed below. Domain rules, model training execution, automatic enforcement hooks and service deployment remain outside its boundary.

## System Overview

Owning component: `sapho-cli`. See the [workspace boundaries](../../spec.md).

## Requirements Architecture

- [FR-033: Inspect a graph without executing it](functional/FR-033.md)
- [FR-034: Invoke bounded graphs from data](functional/FR-034.md)
- [FR-035: Record and replay CLI evaluations](functional/FR-035.md)
- [FR-079: Expand a declared literal grid into validated candidate graphs](functional/FR-079.md)
- [FR-080: Publish sweep candidates without overwriting artifacts](functional/FR-080.md)
- [FR-084: Write a development calibration literal from measured predictions](functional/FR-084.md)

- [FR-045](functional/FR-045.md): host provider credentials
- [FR-046](functional/FR-046.md): shared CLI foundations
- [FR-055](functional/FR-055.md): the `ollama` provider for local models
- [FR-087](functional/FR-087.md): the verified `decisions` provider binding and credentials
- [FR-056](functional/FR-056.md): explicit ceilings for Datasets of about fifteen thousand cases
- [FR-060: Invoke and report shadow comparisons](functional/FR-060.md)
- [FR-064: Write a model roster from a measured split](functional/FR-064.md)
- [FR-072](functional/FR-072.md): per-binding System One host service configuration
- [FR-073](functional/FR-073.md): distinct generic service dispatch and replay compatibility
- [IT-013](integration/IT-013.md): two-port capture, third-service configuration and offline replay
- [FR-078: Publish validated router and escalation graphs](functional/FR-078.md)
- [FR-090](functional/FR-090.md): required-only Claude binding and offline replay
- [FR-071](functional/FR-071.md): held-out promotion gate host and exit policy

## References

The [workspace contract](../../spec.md) and the [shared CLI/evidence limits](../cli/non-functional/NFR-005.md).
