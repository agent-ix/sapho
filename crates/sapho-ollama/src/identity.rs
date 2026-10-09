// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Check model availability before sending an inference request (FR-052).
use crate::http::{Server, failure};
use sapho_core::{ErrorCode, ExtractError};

/// An unknown model is refused before generate or embed. The description is not
/// treated as evidence of which weights will answer a later request.
pub(crate) async fn ensure_installed(server: &Server, model: &str) -> Result<(), ExtractError> {
    let body = serde_json::to_string(&serde_json::json!({ "model": model })).map_err(|_| {
        failure(
            ErrorCode::Config,
            "request_unserializable",
            "Model request is not serializable",
        )
    })?;
    let reply = server.post("api/show", &body).await?;
    if reply.status == 404 {
        return Err(failure(
            ErrorCode::Config,
            "model_not_installed",
            "The server does not have the model",
        ));
    }
    if !reply.success() {
        return Err(reply.status_error());
    }
    Ok(())
}
