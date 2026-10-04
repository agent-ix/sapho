// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Durable lifecycle integration boundaries.
use sapho_campaign::{
    ErrorCode,
    lifecycle::{AttemptState, Campaign, Job, JobId, StageId},
};
fn fixture() -> (tempfile::TempDir, Campaign, JobId, StageId) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Campaign::open(dir.path(), true).unwrap();
    let job = JobId::new("synthetic").unwrap();
    let stage = StageId::new("read").unwrap();
    c.admit(&Job {
        schema: 1,
        id: job.clone(),
        adapter: "test".into(),
        payload_schema: 1,
        payload: serde_json::json!({"source":"synthetic"}),
    })
    .unwrap();
    (dir, c, job, stage)
}
#[test]
fn interrupted_stage_needs_explicit_retry_and_retains_denominator() {
    // Trace: FR-050-AC-2, FR-050-AC-4
    let (_dir, mut c, j, s) = fixture();
    let first = c.start(&j, &s, b"request", None).unwrap();
    assert_eq!(c.recover().unwrap(), 1);
    assert_eq!(c.attempt(first).unwrap().state, AttemptState::Indeterminate);
    assert!(c.start(&j, &s, b"request", None).is_err());
    assert!(c.start(&j, &s, b"request", Some((first, ""))).is_err());
    let retry = c
        .start(&j, &s, b"request", Some((first, "operator reviewed")))
        .unwrap();
    c.finish(retry, AttemptState::Failed, b"failure").unwrap();
    let snapshot = c.snapshot().unwrap();
    assert_eq!(
        snapshot.attempts.get(&AttemptState::Indeterminate),
        Some(&1)
    );
    assert_eq!(snapshot.attempts.get(&AttemptState::Failed), Some(&1));
}
#[test]
fn executed_result_recovers_without_inference_and_seals_atomically() {
    // Trace: FR-050-AC-3, FR-049-AC-3
    let (dir, mut c, j, s) = fixture();
    let id = c.start(&j, &s, b"request", None).unwrap();
    c.finish(id, AttemptState::Executed, b"result").unwrap();
    drop(c);
    let mut c = Campaign::open(dir.path(), true).unwrap();
    assert_eq!(c.recover().unwrap(), 0);
    assert_eq!(c.attempt(id).unwrap().state, AttemptState::Executed);
    c.ledger_mut()
        .connection_mut()
        .unwrap()
        .execute_batch("CREATE TABLE synthetic_domain(value TEXT)")
        .unwrap();
    let result = c.seal(id, |tx, _| {
        tx.execute("INSERT INTO synthetic_domain VALUES('accepted')", [])?;
        Err(sapho_campaign::Error::new(
            ErrorCode::Refused,
            "fault injection",
        ))
    });
    assert!(result.is_err());
    assert_eq!(c.attempt(id).unwrap().state, AttemptState::Executed);
    c.seal(id, |tx, _| {
        tx.execute("INSERT INTO synthetic_domain VALUES('accepted')", [])?;
        Ok(())
    })
    .unwrap();
    assert_eq!(c.attempt(id).unwrap().state, AttemptState::Completed);
    assert_eq!(
        c.ledger()
            .connection()
            .query_row("SELECT count(*) FROM synthetic_domain", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn stage_identity_separates_primary_and_auxiliary_outcomes() {
    // Trace: FR-050-AC-2, FR-051-AC-2
    let (_dir, mut c, j, s) = fixture();
    let id = c.start(&j, &s, b"request", None).unwrap();
    c.finish(id, AttemptState::Executed, b"result").unwrap();
    c.seal(id, |_, _| Ok(())).unwrap();
    let other = StageId::new("independent").unwrap();
    let aux = c
        .start(&j, &other, b"own source first request", None)
        .unwrap();
    c.recover().unwrap();
    assert_eq!(c.attempt(id).unwrap().state, AttemptState::Completed);
    assert_eq!(c.attempt(aux).unwrap().state, AttemptState::Indeterminate);
    c.start(
        &j,
        &other,
        b"own source first request",
        Some((aux, "explicit auxiliary retry")),
    )
    .unwrap();
    assert!(
        c.start(
            &j,
            &s,
            b"request",
            Some((id, "completed stages cannot retry"))
        )
        .is_err()
    );
}
#[test]
fn pause_survives_progress_and_control_replay_has_no_effect() {
    // Trace: FR-052-AC-2
    use sapho_campaign::control::{Control, ControlOutcome};
    let (_dir, mut c, j, s) = fixture();
    let revision = c.snapshot().unwrap().control_revision;
    let control = Control::Pause {
        paused: true,
        revision,
    };
    let attempt = c.start(&j, &s, b"progress", None).unwrap();
    c.capture(attempt, "provider_receipt", b"progress").unwrap();
    assert_eq!(c.apply_control(&control).unwrap(), ControlOutcome::Applied);
    assert!(c.snapshot().unwrap().paused);
    assert_eq!(
        c.apply_control(&control).unwrap(),
        ControlOutcome::AlreadyHandled
    );
    assert_eq!(
        c.apply_control(&Control::Pause {
            paused: false,
            revision
        })
        .unwrap(),
        ControlOutcome::Stale
    );
    assert!(c.snapshot().unwrap().paused);
}
#[test]
fn migration_preserves_ids_events_and_retry_lineage_without_source_changes() {
    // Trace: FR-049-AC-4
    let (dir, mut source, j, s) = fixture();
    let original = source.start(&j, &s, b"first request", None).unwrap();
    source.recover().unwrap();
    let retry = source
        .start(&j, &s, b"retry", Some((original, "explicit retry")))
        .unwrap();
    source
        .finish(retry, AttemptState::Failed, b"retained failure")
        .unwrap();
    let before = source.snapshot().unwrap();
    let target =
        sapho_campaign::migration::migrate(&source, &dir.path().join("migrated"), |_, _| Ok(()))
            .unwrap();
    assert_eq!(target.snapshot().unwrap().sequence, before.sequence);
    assert_eq!(target.snapshot().unwrap().attempts, before.attempts);
    assert_eq!(
        target.attempt(original).unwrap().state,
        AttemptState::Indeterminate
    );
    assert_eq!(
        target.attempt(retry).unwrap().response,
        source.attempt(retry).unwrap().response
    );
    assert_eq!(source.snapshot().unwrap().sequence, before.sequence);
    assert_eq!(
        target
            .ledger()
            .connection()
            .query_row(
                "SELECT parent FROM campaign_attempts WHERE id=?",
                [retry.get()],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        original.get()
    );
    assert!(
        sapho_campaign::migration::migrate(&source, &dir.path().join("migrated"), |_, _| Ok(()))
            .is_err()
    );
}
#[test]
fn dispatch_extension_rolls_back_both_intent_and_domain_transition() {
    // Trace: FR-049-AC-3, FR-050-AC-1
    let (_dir, mut c, j, s) = fixture();
    c.ledger_mut().connection_mut().unwrap().execute_batch("CREATE TABLE stage_owner(job TEXT,state TEXT); INSERT INTO stage_owner VALUES('synthetic','pending')").unwrap();
    let before = c.snapshot().unwrap().sequence;
    let result = c.start_with(&j, &s, b"request", None, |tx, _| {
        tx.execute("UPDATE stage_owner SET state='running'", [])?;
        Err(sapho_campaign::Error::new(
            ErrorCode::Refused,
            "domain precondition refused",
        ))
    });
    assert!(result.is_err());
    assert!(c.attempts().unwrap().is_empty());
    assert_eq!(c.snapshot().unwrap().sequence, before);
    assert_eq!(
        c.ledger()
            .connection()
            .query_row("SELECT state FROM stage_owner", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "pending"
    );
    c.start_with(&j, &s, b"request", None, |tx, _| {
        tx.execute("UPDATE stage_owner SET state='running'", [])?;
        Ok(())
    })
    .unwrap();
    assert_eq!(c.attempts().unwrap().len(), 1);
}
#[test]
fn recovery_domain_refusal_rolls_back_all_abandoned_intents() {
    // Trace: FR-049-AC-3, FR-050-AC-2
    let (_dir, mut c, j, s) = fixture();
    let id = c.start(&j, &s, b"request", None).unwrap();
    let other = c
        .start(&j, &StageId::new("other").unwrap(), b"other request", None)
        .unwrap();
    let before = c.snapshot().unwrap().sequence;
    let result = c.recover_with(|_, a| {
        if a.id == other {
            Err(sapho_campaign::Error::new(
                ErrorCode::Refused,
                "domain recovery refused",
            ))
        } else {
            Ok(())
        }
    });
    assert!(result.is_err());
    assert_eq!(c.attempt(id).unwrap().state, AttemptState::DispatchIntent);
    assert_eq!(
        c.attempt(other).unwrap().state,
        AttemptState::DispatchIntent
    );
    assert_eq!(c.snapshot().unwrap().sequence, before);
    assert_eq!(c.recover().unwrap(), 2);
}
#[test]
fn terminal_projection_failure_preserves_dispatch_for_explicit_recovery() {
    // Trace: FR-049-AC-3, FR-050-AC-4
    let (_dir, mut c, j, s) = fixture();
    let id = c.start(&j, &s, b"request", None).unwrap();
    let before = c.snapshot().unwrap().sequence;
    assert!(
        c.finish_with(id, AttemptState::Failed, b"raw failure", |_, _, _| Err(
            sapho_campaign::Error::new(ErrorCode::Refused, "domain projection refused")
        ))
        .is_err()
    );
    assert_eq!(c.attempt(id).unwrap().state, AttemptState::DispatchIntent);
    assert_eq!(c.snapshot().unwrap().sequence, before);
    c.finish_with(id, AttemptState::Failed, b"raw failure", |_, a, state| {
        assert_eq!(a.stage, s);
        assert_eq!(state, AttemptState::Failed);
        Ok(())
    })
    .unwrap();
    assert_eq!(c.attempt(id).unwrap().state, AttemptState::Failed);
}

#[test]
fn migration_copies_only_snapshot_declared_domain_artifacts_and_prevalidates_hashes() {
    // Trace: FR-049-AC-2, FR-049-AC-4, FR-051-AC-2
    let (dir, mut source, _, _) = fixture();
    let domain_hash = source.ledger().blob(b"domain only").unwrap();
    let neighbor = source
        .ledger()
        .blob(b"unreferenced protected-neighbor stand-in")
        .unwrap();
    source
        .ledger_mut()
        .connection_mut()
        .unwrap()
        .execute_batch("CREATE TABLE domain_artifacts(hash TEXT NOT NULL)")
        .unwrap();
    source
        .ledger_mut()
        .connection_mut()
        .unwrap()
        .execute("INSERT INTO domain_artifacts VALUES(?)", [&domain_hash])
        .unwrap();
    let output = dir.path().join("domain-migration");
    let target = sapho_campaign::migration::migrate_with_references(
        &source,
        &output,
        |tx| {
            let mut q = tx.prepare("SELECT hash FROM domain_artifacts")?;
            Ok(q.query_map([], |r| r.get(0))?
                .collect::<std::result::Result<Vec<String>, _>>()?)
        },
        |from, to| {
            to.execute_batch("CREATE TABLE domain_artifacts(hash TEXT NOT NULL)")?;
            let hash: String =
                from.query_row("SELECT hash FROM domain_artifacts", [], |r| r.get(0))?;
            to.execute("INSERT INTO domain_artifacts VALUES(?)", [hash])?;
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(target.ledger().load(&domain_hash).unwrap(), b"domain only");
    assert!(!output.join("blobs").join(neighbor).exists());
    std::fs::write(dir.path().join("blobs").join(&domain_hash), b"corrupt").unwrap();
    let failed = dir.path().join("refused-corruption");
    assert!(
        sapho_campaign::migration::migrate_with_references(
            &source,
            &failed,
            |_| Ok(vec![domain_hash]),
            |_, _| Ok(())
        )
        .is_err()
    );
    assert!(!failed.exists());
}
