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
