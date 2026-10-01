// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! RFC 6901 selection with whole-document evidence attribution.
use crate::{SelectionError, source};
use sapho_core::{Inputs, ValueType, decode_json, decode_plain, validate_name};
use serde_json::Value as Json;

/// Project a strict JSON document under an explicit schema; no type inference or model work.
pub fn select_json(
    bytes: &[u8],
    pointer: &str,
    schema: &ValueType,
    id_prefix: &str,
    port: &str,
    location: &str,
    max_bytes: usize,
) -> Result<Inputs, SelectionError> {
    validate_name(port)?;
    let document: Json = decode_json(bytes, max_bytes)?;
    let mut selected = &document;
    if !pointer.is_empty() {
        let rest = pointer
            .strip_prefix('/')
            .ok_or_else(|| SelectionError::PointerSyntax(pointer.into()))?;
        for token in rest.split('/') {
            let mut decoded = String::new();
            let mut chars = token.chars();
            while let Some(c) = chars.next() {
                if c == '~' {
                    match chars.next() {
                        Some('0') => decoded.push('~'),
                        Some('1') => decoded.push('/'),
                        _ => return Err(SelectionError::PointerSyntax(pointer.into())),
                    }
                } else {
                    decoded.push(c);
                }
            }
            selected = match selected {
                Json::Object(fields) => fields
                    .get(&decoded)
                    .ok_or_else(|| SelectionError::MissingKey(decoded.clone()))?,
                Json::Array(items) => {
                    if decoded.is_empty()
                        || (decoded.len() > 1 && decoded.starts_with('0'))
                        || !decoded.bytes().all(|b| b.is_ascii_digit())
                    {
                        return Err(SelectionError::ArrayIndex(decoded));
                    }
                    let index = decoded
                        .parse::<usize>()
                        .map_err(|_| SelectionError::ArrayIndex(decoded.clone()))?;
                    items
                        .get(index)
                        .ok_or(SelectionError::ArrayIndex(decoded))?
                }
                Json::Null | Json::Bool(_) | Json::Number(_) | Json::String(_) => {
                    return Err(SelectionError::PointerType);
                }
            }
        }
    }
    let attribution = source("json", location, bytes)?;
    let datum = decode_plain(id_prefix, schema, selected, &[attribution])?;
    Ok(Inputs::from([(port.into(), datum)]))
}
