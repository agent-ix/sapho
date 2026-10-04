// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Storage safety and trusted extension transaction boundaries.
use sapho_campaign::{ErrorCode, storage::Ledger};
#[test]
fn fence_readers_and_atomic_domain_extension() {
    // Trace: FR-049-AC-1, FR-049-AC-3
    let dir = tempfile::tempdir().unwrap();
    let mut writer = Ledger::open(dir.path(), "campaign.sqlite", true, 128).unwrap();
    assert_eq!(
        Ledger::open(dir.path(), "campaign.sqlite", true, 128)
            .err()
            .unwrap()
            .code,
        ErrorCode::Conflict
    );
    let hash = writer.blob(b"evidence").unwrap();
    {
        let tx = writer.connection_mut().unwrap().transaction().unwrap();
        tx.execute_batch("CREATE TABLE domain_result(hash TEXT)")
            .unwrap();
        tx.execute("INSERT INTO domain_result VALUES(?)", [&hash])
            .unwrap();
        Ledger::event_tx(&tx, "sealed", "job", &hash).unwrap();
        tx.commit().unwrap();
    }
    let reader = Ledger::open(dir.path(), "campaign.sqlite", false, 128).unwrap();
    assert_eq!(reader.load(&hash).unwrap(), b"evidence");
    assert_eq!(
        reader
            .connection()
            .query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        reader.blob(b"no mutation").unwrap_err().code,
        ErrorCode::Refused
    );
}
#[test]
fn corrupt_oversized_and_symlink_artifacts_refuse() {
    // Trace: FR-049-AC-2
    let dir = tempfile::tempdir().unwrap();
    let store = Ledger::open(dir.path(), "campaign.sqlite", true, 8).unwrap();
    assert_eq!(
        store.blob(b"123456789").unwrap_err().code,
        ErrorCode::Refused
    );
    let hash = store.blob(b"valid").unwrap();
    std::fs::write(dir.path().join("blobs").join(&hash), b"wrong").unwrap();
    assert_eq!(store.load(&hash).unwrap_err().code, ErrorCode::Storage);
    #[cfg(unix)]
    {
        let outside = dir.path().join("outside");
        std::fs::write(&outside, b"source").unwrap();
        let h = sapho_campaign::storage::digest(b"source");
        std::os::unix::fs::symlink(&outside, dir.path().join("blobs").join(&h)).unwrap();
        assert!(store.load(&h).is_err());
    }
}
#[test]
fn domain_and_completion_event_roll_back_together() {
    // Trace: FR-049-AC-3
    let dir = tempfile::tempdir().unwrap();
    let mut store = Ledger::open(dir.path(), "campaign.sqlite", true, 128).unwrap();
    store
        .connection_mut()
        .unwrap()
        .execute_batch("CREATE TABLE domain_result(hash TEXT)")
        .unwrap();
    let hash = store.blob(b"valid").unwrap();
    {
        let tx = store.connection_mut().unwrap().transaction().unwrap();
        tx.execute("INSERT INTO domain_result VALUES (?)", [&hash])
            .unwrap();
        Ledger::event_tx(&tx, "sealed", "job", &hash).unwrap();
    }
    assert_eq!(
        store
            .connection()
            .query_row("SELECT count(*) FROM domain_result", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        store
            .connection()
            .query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}
