// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Relevant-revision controls applied exactly once by the fenced writer.
use crate::{
    Error, ErrorCode, Result,
    lifecycle::{AttemptId, AttemptState, Campaign},
    storage::Ledger,
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
/// Guarded operator request, persisted separately from unrelated execution progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Control {
    /// Change the operator pause state at a safe stage boundary.
    Pause {
        /// Desired pause state.
        paused: bool,
        /// Last observed control revision, not the global event sequence.
        revision: i64,
    },
    /// Authorize one explicit retry of the current failed stage attempt.
    Retry {
        /// Latest attempt for this job/stage.
        attempt: AttemptId,
        /// State observed by the operator.
        expected: AttemptState,
        /// Nonempty operator reason.
        reason: String,
    },
}
/// Receipt separates accepted control submission from actual application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlOutcome {
    /// Request applied transactionally.
    Applied,
    /// A matching receipt exists; no repeated effect.
    AlreadyHandled,
    /// Relevant state changed, requiring a fresh operator decision.
    Stale,
}
impl Campaign {
    /// Apply one exact control identity and retain an immutable receipt in the same transaction.
    pub fn apply_control(&mut self, control: &Control) -> Result<ControlOutcome> {
        let bytes = sapho_core::bounded_json(control, 4096)
            .map_err(|e| Error::new(ErrorCode::Invalid, e.to_string()))?;
        let hash = self.ledger().blob(&bytes)?;
        let tx = self.ledger_mut().connection_mut()?.transaction()?;
        if tx
            .query_row(
                "SELECT seq FROM events WHERE kind='campaign_control_receipt' AND subject=?",
                [&hash],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
            .is_some()
        {
            return Ok(ControlOutcome::AlreadyHandled);
        }
        let outcome = match control {
            Control::Pause { paused, revision } => {
                if *revision < 0 {
                    return Err(Error::new(ErrorCode::Invalid, "negative control revision"));
                }
                let changed=tx.execute("UPDATE campaign_control SET paused=?,revision=revision+1 WHERE id=1 AND revision=?",params![paused,revision])?;
                if changed == 1 {
                    ControlOutcome::Applied
                } else {
                    ControlOutcome::Stale
                }
            }
            Control::Retry {
                attempt,
                expected,
                reason,
            } => {
                if reason.trim().is_empty() || !expected.retryable() {
                    return Err(Error::new(
                        ErrorCode::Invalid,
                        "retry needs nonempty reason and non-success state",
                    ));
                }
                let actual: Option<(String, String, String)> = tx
                    .query_row(
                        "SELECT job,stage,state FROM campaign_attempts WHERE id=?",
                        [attempt.get()],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )
                    .optional()?;
                match actual {
                    Some((job, stage, state)) if AttemptState::parse(&state)? == *expected => {
                        let latest: i64 = tx.query_row(
                            "SELECT max(id) FROM campaign_attempts WHERE job=? AND stage=?",
                            params![job, stage],
                            |r| r.get(0),
                        )?;
                        if latest == attempt.get() {
                            Ledger::event_tx(
                                &tx,
                                "campaign_retry_authorized",
                                &attempt.get().to_string(),
                                &hash,
                            )?;
                            ControlOutcome::Applied
                        } else {
                            ControlOutcome::Stale
                        }
                    }
                    _ => ControlOutcome::Stale,
                }
            }
        };
        let kind = match outcome {
            ControlOutcome::Applied => "campaign_control_applied",
            ControlOutcome::Stale => "campaign_control_stale",
            ControlOutcome::AlreadyHandled => {
                return Err(Error::new(ErrorCode::Storage, "invalid receipt transition"));
            }
        };
        Ledger::event_tx(&tx, kind, &hash, &hash)?;
        Ledger::event_tx(&tx, "campaign_control_receipt", &hash, &hash)?;
        tx.commit()?;
        Ok(outcome)
    }
    /// Read the retained explicit retry authorization for a current attempt.
    pub fn retry_authorization(&self, attempt: AttemptId) -> Result<Option<String>> {
        let hash:Option<String>=self.ledger().connection().query_row("SELECT evidence FROM events WHERE kind='campaign_retry_authorized' AND subject=? ORDER BY seq DESC LIMIT 1",[attempt.get().to_string()],|r|r.get(0)).optional()?;
        let Some(hash) = hash else { return Ok(None) };
        let control: Control = serde_json::from_slice(&self.ledger().load(&hash)?)
            .map_err(|e| Error::new(ErrorCode::Storage, e.to_string()))?;
        match control {
            Control::Retry {
                attempt: target,
                reason,
                ..
            } if target == attempt => Ok(Some(reason)),
            _ => Err(Error::new(
                ErrorCode::Storage,
                "retry receipt does not bind attempt",
            )),
        }
    }
}

/// Queue a control without acquiring writer ownership. Submission is not application.
pub fn submit(root: &std::path::Path, control: &Control) -> Result<String> {
    use std::{
        fs::{File, OpenOptions},
        io::Write,
    };
    let root = root.canonicalize()?;
    let dir = root.join("campaign-controls");
    match std::fs::create_dir(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    }
    if !std::fs::symlink_metadata(&dir)?.is_dir() {
        return Err(Error::new(
            ErrorCode::Refused,
            "control directory is not a real directory",
        ));
    }
    let bytes = sapho_core::bounded_json(control, 4096)
        .map_err(|e| Error::new(ErrorCode::Invalid, e.to_string()))?;
    let hash = crate::storage::digest(&bytes);
    let path = dir.join(format!("{hash}.json"));
    if path.try_exists()? {
        if crate::storage::read_file(&path, 4096)? != bytes {
            return Err(Error::new(ErrorCode::Storage, "control identity mismatch"));
        }
        return Ok(hash);
    }
    let temp = dir.join(format!(".{hash}-{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    std::fs::rename(&temp, &path)?;
    File::open(dir)?.sync_all()?;
    Ok(hash)
}
/// Apply queued controls at a stage boundary, preserving durable receipts before removal.
pub fn drain(campaign: &mut Campaign) -> Result<()> {
    let dir = campaign.ledger().root().join("campaign-controls");
    if !dir.try_exists()? {
        return Ok(());
    }
    if !std::fs::symlink_metadata(&dir)?.is_dir() {
        return Err(Error::new(
            ErrorCode::Refused,
            "control directory is not a real directory",
        ));
    }
    let mut entries = std::fs::read_dir(&dir)?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.ends_with(".json") {
            continue;
        }
        let bytes = crate::storage::read_file(&entry.path(), 4096)?;
        let hash = crate::storage::digest(&bytes);
        if name != format!("{hash}.json") {
            return Err(Error::new(
                ErrorCode::Refused,
                "control filename hash mismatch",
            ));
        }
        let control: Control = serde_json::from_slice(&bytes)
            .map_err(|e| Error::new(ErrorCode::Invalid, e.to_string()))?;
        campaign.apply_control(&control)?;
        std::fs::remove_file(entry.path())?;
    }
    std::fs::File::open(dir)?.sync_all()?;
    Ok(())
}
