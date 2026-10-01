---
id: SR-013
title: "Approximate-distribution verification evidence"
type: SpecReview
analysis: base
scope: "Focused distribution-policy change and local gate evidence"
review_set: subset
---
# Approximate-distribution verification

## Summary

Reviewed source 9865821b16b0ba1162ef6110e2e10b485d05f0ba. Full pre-PR make ci passed (exit0). Sapho40 offline behavior tests plus1 compiled doc example in both all-feature and no-default-feature lanes. Formatting, Clippy -D warnings, dependency/supply-chain checks, docs, Quire spec/review validation and unsafe audit passed. Sapho’s advisory cache now lives under its owned target directory; no checks were disabled. The original strict distribution assessment remains unchanged. Final-source make ci passed after the compact-context/HTTP400 change. The last pre-merge make ci gate also passed (exit0); final-source full gates passed twice, including spec/review validation and replay tests. No Kani lane is present in these repositories. Three development passages completed all four Jev stages with exact saved Evaluation replay; ten approximate complete distributions were retained unchanged and accepted under explicit0.01 allowance. A separate token-limit refusal was addressed by removing repeated question definitions, without cropping original source or prior raw answers. These are operational checks, not semantic accuracy or calibration measurements. Private data/recordings stay outside Git.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | low | No findings (placeholder) | - |
