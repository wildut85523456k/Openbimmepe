//! AUTO-002 acceptance, from `04b` §3.1 and §7.1: pending handles, dependency
//! groups and atomic apply. Scaled down from the doc's 500-element cases so the
//! suite stays fast, but the same invariants.
use openbim_document::{Category, ElementId};
use openbim_automation::engine::{
    op_group, ApplyError, CancelError, CommitError, Engine, LocalId, PendingHandle, ProposalId,
    ProposalState, Resolved, RunId,
};
use std::collections::BTreeMap;

fn handle(run: RunId, proposal: u64, local: u64, category: Category) -> PendingHandle {
    PendingHandle::new(run, ProposalId(proposal), LocalId(local), category)
}

/// Commit that succeeds and invents an element id per local id.
fn ok_commit(group: &[PendingHandle]) -> Result<Vec<Resolved>, CommitError> {
    Ok(group
        .iter()
        .map(|h| Resolved { local_id: h.local_id, element: ElementId(1000 + h.local_id.0) })
        .collect())
}

/// Commit that always fails.
fn bad_commit(_group: &[PendingHandle]) -> Result<Vec<Resolved>, CommitError> {
    Err(CommitError::RevisionConflict { expected: 1, actual: 2 })
}

fn engine(run: RunId, category: Category, handles: Vec<PendingHandle>) -> Engine {
    Engine::new(run, ProposalId(0), category, handles).expect("built proposal")
}

#[test]
fn wrong_run_handle_is_refused() {
    let run = RunId::new();
    let other = RunId::new();
    let h = handle(other, 0, 1, Category::Wall);
    let err = Engine::new(run, ProposalId(0), Category::Wall, vec![h]).unwrap_err();
    assert!(matches!(err, ApplyError::WrongRun { .. }));
}

#[test]
fn wrong_type_handle_is_refused() {
    let run = RunId::new();
    let h = handle(run, 0, 1, Category::Beam);
    let err = Engine::new(run, ProposalId(0), Category::Wall, vec![h]).unwrap_err();
    assert!(matches!(err, ApplyError::WrongType { .. }));
}

#[test]
fn wrong_proposal_handle_is_refused() {
    let run = RunId::new();
    let h = handle(run, 7, 1, Category::Wall);
    let err = Engine::new(run, ProposalId(0), Category::Wall, vec![h]).unwrap_err();
    assert!(matches!(err, ApplyError::WrongProposal { .. }));
}

#[test]
fn apply_valid_group_commits_and_advances() {
    let run = RunId::new();
    let h = handle(run, 0, 1, Category::Wall);
    let mut e = engine(run, Category::Wall, vec![h]);
    let hs = e.handles();
    let out = e.apply(&hs, ok_commit).unwrap();
    assert_eq!(e.state(), ProposalState::Applied);
    assert_eq!(out.resolved.len(), 1);
    assert_eq!(out.resolved[0].element, ElementId(1001));
}

#[test]
fn apply_again_is_refused() {
    let run = RunId::new();
    let h = handle(run, 0, 1, Category::Wall);
    let mut e = engine(run, Category::Wall, vec![h]);
    let hs = e.handles();
    let _ = e.apply(&hs, ok_commit).unwrap();
    let hs = e.handles();
    let err = e.apply(&hs, ok_commit).unwrap_err();
    assert_eq!(err, ApplyError::NotOpen(ProposalState::Applied));
}

#[test]
fn cancel_makes_the_proposal_uncommittable() {
    let run = RunId::new();
    let h = handle(run, 0, 1, Category::Wall);
    let mut e = engine(run, Category::Wall, vec![h]);
    e.cancel().unwrap();
    let hs = e.handles();
    let err = e.apply(&hs, ok_commit).unwrap_err();
    assert_eq!(err, ApplyError::NotOpen(ProposalState::Cancelled));
}

#[test]
fn cancel_twice_is_refused() {
    let run = RunId::new();
    let h = handle(run, 0, 1, Category::Wall);
    let mut e = engine(run, Category::Wall, vec![h]);
    e.cancel().unwrap();
    assert_eq!(e.cancel().unwrap_err(), CancelError::NotCancellable(ProposalState::Cancelled));
}

#[test]
fn unknown_handle_is_refused() {
    let run = RunId::new();
    let h = handle(run, 0, 1, Category::Wall);
    let mut e = engine(run, Category::Wall, vec![h]);
    let foreign = handle(run, 0, 99, Category::Wall);
    let err = e.apply(&[foreign], ok_commit).unwrap_err();
    assert_eq!(err, ApplyError::UnknownHandle(LocalId(99)));
}

#[test]
fn stale_prerequisite_refuses_whole_group() {
    let run = RunId::new();
    let host = handle(run, 0, 1, Category::Wall);
    let fitting = handle(run, 0, 2, Category::Wall);
    let mut e = engine(run, Category::Wall, vec![host, fitting]);
    e.mark_stale(LocalId(1));
    let group = vec![host, fitting];
    let err = e.apply(&group, ok_commit).unwrap_err();
    assert_eq!(err, ApplyError::Stale(LocalId(1)));
    assert_eq!(e.state(), ProposalState::Open);
}

#[test]
fn locked_member_refuses_whole_group() {
    let run = RunId::new();
    let a = handle(run, 0, 1, Category::Wall);
    let b = handle(run, 0, 2, Category::Wall);
    let mut e = engine(run, Category::Wall, vec![a, b]);
    e.mark_locked(LocalId(2));
    let err = e.apply(&[a, b], ok_commit).unwrap_err();
    assert_eq!(err, ApplyError::Locked(LocalId(2)));
    assert_eq!(e.state(), ProposalState::Open);
}

#[test]
fn commit_failure_leaves_proposal_open() {
    let run = RunId::new();
    let h = handle(run, 0, 1, Category::Wall);
    let mut e = engine(run, Category::Wall, vec![h]);
    let hs = e.handles();
    let err = e.apply(&hs, bad_commit).unwrap_err();
    assert!(matches!(err, ApplyError::Commit(CommitError::RevisionConflict { .. })));
    assert_eq!(e.state(), ProposalState::Open);
}

#[test]
fn partial_application_then_completion() {
    let run = RunId::new();
    let a = handle(run, 0, 1, Category::Wall);
    let b = handle(run, 0, 2, Category::Wall);
    let mut e = engine(run, Category::Wall, vec![a, b]);
    let _ = e.apply(&[a], ok_commit).unwrap();
    assert_eq!(e.state(), ProposalState::PartiallyApplied);
    let _ = e.apply(&[b], ok_commit).unwrap();
    assert_eq!(e.state(), ProposalState::Applied);
}

#[test]
fn op_group_merges_overlapping_dependencies_transitively() {
    let run = RunId::new();
    let handles = vec![
        handle(run, 0, 1, Category::Wall),
        handle(run, 0, 2, Category::Wall),
        handle(run, 0, 3, Category::Wall),
        handle(run, 0, 4, Category::Wall),
    ];
    // 1 <- 2 <- 3 is a chain; 4 is independent.
    let mut deps = BTreeMap::new();
    deps.insert(LocalId(2), vec![LocalId(1)]);
    deps.insert(LocalId(3), vec![LocalId(2)]);
    let groups = op_group(&handles, &deps);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].len(), 3);
    assert_eq!(groups[0].iter().map(|h| h.local_id.0).collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(groups[1].len(), 1);
    assert_eq!(groups[1][0].local_id.0, 4);
}

#[test]
fn op_group_keeps_independent_creations_separate() {
    let run = RunId::new();
    let handles: Vec<PendingHandle> = (1..=5).map(|i| handle(run, 0, i, Category::Wall)).collect();
    let groups = op_group(&handles, &BTreeMap::new());
    assert_eq!(groups.len(), 5);
}

#[test]
fn op_group_ignores_dependencies_on_absent_ids() {
    let run = RunId::new();
    let handles = vec![handle(run, 0, 1, Category::Wall)];
    let mut deps = BTreeMap::new();
    deps.insert(LocalId(1), vec![LocalId(999)]);
    let groups = op_group(&handles, &deps);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].len(), 1);
}

#[test]
fn independent_500_case_applies_499_and_refuses_the_locked_one() {
    let run = RunId::new();
    let handles: Vec<PendingHandle> = (1..=500)
        .map(|i| handle(run, 0, i, Category::Wall))
        .collect();
    let mut e = engine(run, Category::Wall, handles.clone());
    e.mark_locked(LocalId(500)); // the one locked by someone else
    // Independent groups: each handle is its own group.
    let groups = op_group(&handles, &BTreeMap::new());
    let mut applied = 0;
    for group in &groups {
        match e.apply(group, ok_commit) {
            Ok(_) => applied += 1,
            Err(ApplyError::Locked(_)) => {}
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }
    assert_eq!(applied, 499);
}
