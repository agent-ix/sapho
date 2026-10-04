// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Explicit snapshot migration into new state, preserving original identities.
use crate::{Result, lifecycle::Campaign};
use rusqlite::{Transaction, params_from_iter, types::Value};
use std::{fs::File, path::Path};
/// Copy the generic ledger plus an explicit trusted domain extension into a new root.
/// The original connection uses a read transaction; no source SQL updates occur.
/// Extension code owns its exact table allowlist and must not scan neighboring files.
pub fn migrate<F>(source: &Campaign, destination: &Path, extension: F) -> Result<Campaign>
where
    F: FnOnce(&Transaction<'_>, &Transaction<'_>) -> Result<()>,
{
    let snapshot = source.ledger().connection().unchecked_transaction()?;
    source.verify()?;
    let references = {
        let mut q=snapshot.prepare("SELECT evidence FROM events UNION SELECT evidence FROM campaign_jobs UNION SELECT request FROM campaign_attempts UNION SELECT response FROM campaign_attempts WHERE response IS NOT NULL ORDER BY 1")?;
        q.query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    std::fs::create_dir(destination)?;
    let mut target = Campaign::open(destination, true)?;
    for hash in references {
        target.ledger().blob(&source.ledger().load(&hash)?)?;
    }
    let tx = target.ledger_mut().connection_mut()?.transaction()?;
    for (select, insert, columns) in [
        (
            "SELECT seq,kind,subject,evidence FROM events ORDER BY seq",
            "INSERT INTO events VALUES(?,?,?,?)",
            4,
        ),
        (
            "SELECT id,adapter,evidence FROM campaign_jobs ORDER BY id",
            "INSERT INTO campaign_jobs VALUES(?,?,?)",
            3,
        ),
        (
            "SELECT id,job,stage,parent,reason,state,request,response FROM campaign_attempts ORDER BY id",
            "INSERT INTO campaign_attempts VALUES(?,?,?,?,?,?,?,?)",
            8,
        ),
        (
            "SELECT id,paused,revision FROM campaign_control",
            "INSERT INTO campaign_control VALUES(?,?,?)",
            3,
        ),
    ] {
        if columns == 3 && select.contains("campaign_control") {
            tx.execute("DELETE FROM campaign_control", [])?;
        }
        let mut query = snapshot.prepare(select)?;
        let rows = query.query_map([], |row| {
            (0..columns)
                .map(|i| row.get::<_, Value>(i))
                .collect::<std::result::Result<Vec<_>, _>>()
        })?;
        for row in rows {
            tx.execute(insert, params_from_iter(row?))?;
        }
    }
    extension(&snapshot, &tx)?;
    tx.commit()?;
    snapshot.commit()?;
    target.verify()?;
    File::open(destination)?.sync_all()?;
    Ok(target)
}
