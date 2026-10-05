// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Model identity: the weights digest read from `/api/show` before each request (FR-052).
use crate::http::{Server, failure};
use sapho_core::{ErrorCode, ExtractError};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Description {
    modelfile: String,
}

/// The digest of the weights named by the `FROM` line, as `sha256:<hex>`.
fn weights_digest(modelfile: &str) -> Option<String> {
    let source = modelfile
        .lines()
        .find_map(|line| line.strip_prefix("FROM "))?
        .trim();
    let blob = source.rsplit(['/', '\\']).next()?.strip_prefix("sha256-")?;
    (!blob.is_empty() && blob.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| format!("sha256:{blob}"))
}

/// Read the model's current weights digest and hold it to the first one `pinned` saw.
///
/// An unknown model is `Config`; a digest that differs from the pinned one is
/// `ModelMismatch`, so a model replaced mid-run never answers under the old identity.
pub(crate) async fn resolve(
    server: &Server,
    model: &str,
    pinned: &OnceLock<String>,
) -> Result<String, ExtractError> {
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
    if !(200..300).contains(&reply.status) {
        return Err(failure(
            ErrorCode::BackendFailed,
            "http_status",
            "Model description request failed",
        ));
    }
    let digest = serde_json::from_str::<Description>(&reply.body)
        .ok()
        .and_then(|d| weights_digest(&d.modelfile))
        .ok_or_else(|| {
            failure(
                ErrorCode::BackendFailed,
                "digest_unavailable",
                "The model description names no weights blob",
            )
        })?;
    if pinned.get_or_init(|| digest.clone()) != &digest {
        return Err(failure(
            ErrorCode::ModelMismatch,
            "weights_changed",
            "The model's weights changed during the run",
        ));
    }
    Ok(digest)
}

#[cfg(test)]
mod tests {
    use super::weights_digest;

    #[test]
    fn digest_comes_from_the_from_blob_only() {
        let file = "# comment\nFROM /root/.ollama/models/blobs/sha256-58574f2e\nTEMPLATE x\n";
        assert_eq!(weights_digest(file).as_deref(), Some("sha256:58574f2e"));
        assert_eq!(
            weights_digest("FROM C:\\m\\blobs\\sha256-ab12").as_deref(),
            Some("sha256:ab12")
        );
        assert_eq!(weights_digest("FROM qwen3:30b"), None);
        assert_eq!(weights_digest("FROM /blobs/sha256-zz"), None);
        assert_eq!(weights_digest("TEMPLATE x"), None);
    }
}
