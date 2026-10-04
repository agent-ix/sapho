// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Narrow evidence bundles from exact ledger references, never neighboring files.
use crate::{Result, lifecycle::Campaign};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
};
/// Immutable event exported with its original sequence and evidence identity.
#[derive(Debug, Serialize)]
pub struct Event {
    /// Original durable sequence.
    pub sequence: i64,
    /// Stable event type.
    pub kind: String,
    /// Exact subject.
    pub subject: String,
    /// Referenced content identity.
    pub evidence: String,
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    Ok(())
}
impl Campaign {
    /// Verify every known evidence reference without scanning neighboring artifacts.
    pub fn verify(&self) -> Result<()> {
        for hash in self.references()? {
            self.ledger().load(&hash)?;
        }
        Ok(())
    }
    fn references(&self) -> Result<Vec<String>> {
        let mut q=self.ledger().connection().prepare("SELECT evidence FROM events UNION SELECT evidence FROM campaign_jobs UNION SELECT request FROM campaign_attempts UNION SELECT response FROM campaign_attempts WHERE response IS NOT NULL ORDER BY 1")?;
        Ok(q.query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }
    /// Export a new verified evidence bundle. Export is neither publication nor domain approval.
    pub fn export(&self, destination: &Path) -> Result<String> {
        self.verify()?;
        let hashes = self.references()?;
        let snapshot = self.snapshot()?;
        let mut q = self
            .ledger()
            .connection()
            .prepare("SELECT seq,kind,subject,evidence FROM events ORDER BY seq")?;
        let events = q
            .query_map([], |r| {
                Ok(Event {
                    sequence: r.get(0)?,
                    kind: r.get(1)?,
                    subject: r.get(2)?,
                    evidence: r.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        std::fs::create_dir(destination)?;
        std::fs::create_dir(destination.join("blobs"))?;
        let mut artifacts = BTreeMap::new();
        for hash in hashes {
            let bytes = self.ledger().load(&hash)?;
            write_new(&destination.join("blobs").join(&hash), &bytes)?;
            artifacts.insert(hash, bytes.len());
        }
        File::open(destination.join("blobs"))?.sync_all()?;
        let bytes=sapho_core::bounded_json(&serde_json::json!({"schema":1,"adapter":"sapho-campaign/v1","snapshot":snapshot,"events":events,"artifacts":artifacts,"publication":"staged_only"}),crate::storage::DEFAULT_ARTIFACT_BYTES).map_err(|e|crate::Error::new(crate::ErrorCode::Refused,e.to_string()))?;
        let hash = crate::storage::digest(&bytes);
        write_new(&destination.join("manifest.json"), &bytes)?;
        File::open(destination)?.sync_all()?;
        Ok(hash)
    }
}
