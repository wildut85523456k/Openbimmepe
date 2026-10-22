//! The apply side of a proposal: which pending creations are committed together,
//! and the state machine that makes a commit atomic (`04b` §3.1).
//!
//! A proposal owns its pending handles. It validates a group of them against its
//! own run, proposal and element type, then commits them through a callback that
//! performs the real atomic write. Only a commit that succeeds moves the
//! proposal forward; a refused or failed group leaves it exactly as it was, so
//! independent groups can still be applied later.
mod group;
mod handle;

pub use group::op_group;
pub use handle::{LocalId, PendingHandle, ProposalId, RunId};

use openbim_document::{Category, ElementId};
use std::collections::{BTreeMap, BTreeSet};

/// Where a proposal is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProposalState {
    /// Evaluated, nothing committed yet.
    Open,
    /// Some groups committed, some handles still pending.
    PartiallyApplied,
    /// Every handle is committed.
    Applied,
    /// Called off by the user; it can never be committed.
    Cancelled,
}

/// Why a group could not be applied. The proposal is left exactly as it was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyError {
    /// The proposal is not open, so nothing may be committed.
    NotOpen(ProposalState),
    /// A handle belongs to another run.
    WrongRun { expected: RunId, actual: RunId },
    /// A handle creates a different kind of element than the proposal does.
    WrongType { expected: Category, actual: Category },
    /// A handle belongs to a different proposal of this run.
    WrongProposal { expected: ProposalId, actual: ProposalId },
    /// A handle is not one this proposal produced.
    UnknownHandle(LocalId),
    /// A prerequisite is no longer there; the whole group is stale.
    Stale(LocalId),
    /// A member is locked by someone else: the whole group becomes a change request.
    Locked(LocalId),
    /// The commit itself failed; nothing was written.
    Commit(CommitError),
}

/// Why a proposal could not be cancelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancelError {
    /// Only an open proposal can be called off.
    NotCancellable(ProposalState),
}

/// Why the atomic write of a validated group failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitError {
    /// The document moved under the proposal.
    RevisionConflict { expected: u64, actual: u64 },
    /// The host refused the write.
    Refused(String),
}

/// A local id resolved to the element it became.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub local_id: LocalId,
    pub element: ElementId,
}

/// The result of a successful group commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyOutcome {
    pub resolved: Vec<Resolved>,
}

/// One proposal's pending creations and the state machine over them.
#[derive(Debug)]
pub struct Engine {
    run_id: RunId,
    proposal_id: ProposalId,
    category: Category,
    state: ProposalState,
    handles: BTreeMap<LocalId, PendingHandle>,
    committed: BTreeSet<LocalId>,
    stale: BTreeSet<LocalId>,
    locked: BTreeSet<LocalId>,
}

impl Engine {
    /// The proposal's run, proposal and element type are fixed here. Handles that
    /// do not match them are rejected, so a proposal cannot be built with a
    /// handle from another run or of the wrong type.
    pub fn new(
        run_id: RunId,
        proposal_id: ProposalId,
        category: Category,
        handles: Vec<PendingHandle>,
    ) -> Result<Self, ApplyError> {
        let mut by_id = BTreeMap::new();
        for handle in handles {
            if handle.run_id != run_id {
                return Err(ApplyError::WrongRun { expected: run_id, actual: handle.run_id });
            }
            if handle.proposal_id != proposal_id {
                return Err(ApplyError::WrongProposal { expected: proposal_id, actual: handle.proposal_id });
            }
            if handle.category != category {
                return Err(ApplyError::WrongType { expected: category, actual: handle.category });
            }
            if by_id.insert(handle.local_id, handle).is_some() {
                return Err(ApplyError::UnknownHandle(handle.local_id));
            }
        }
        Ok(Self {
            run_id,
            proposal_id,
            category,
            state: ProposalState::Open,
            handles: by_id,
            committed: BTreeSet::new(),
            stale: BTreeSet::new(),
            locked: BTreeSet::new(),
        })
    }

    pub fn state(&self) -> ProposalState {
        self.state
    }

    pub fn run_id(&self) -> RunId {
        self.run_id
    }

    pub fn proposal_id(&self) -> ProposalId {
        self.proposal_id
    }

    pub fn category(&self) -> Category {
        self.category
    }

    /// Every pending handle of this proposal, in a stable order.
    pub fn handles(&self) -> Vec<PendingHandle> {
        self.handles.values().copied().collect()
    }

    pub fn is_stale(&self, id: LocalId) -> bool {
        self.stale.contains(&id)
    }

    pub fn is_locked(&self, id: LocalId) -> bool {
        self.locked.contains(&id)
    }

    /// A prerequisite is no longer there. Its whole group is refused on apply.
    pub fn mark_stale(&mut self, id: LocalId) {
        self.stale.insert(id);
    }

    /// A member is locked by someone else (`COLLAB-002`). Its whole group
    /// becomes a change request on apply.
    pub fn mark_locked(&mut self, id: LocalId) {
        self.locked.insert(id);
    }

    /// Validate one group and, when it is good, commit it through `commit`.
    ///
    /// `commit` performs the real atomic write and returns the local ids
    /// resolved to the elements they became. Only a commit that succeeds moves
    /// the proposal forward; a refused or failed group leaves the proposal open
    /// so other groups can still be applied.
    pub fn apply<F>(
        &mut self,
        group: &[PendingHandle],
        commit: F,
    ) -> Result<ApplyOutcome, ApplyError>
    where
        F: FnOnce(&[PendingHandle]) -> Result<Vec<Resolved>, CommitError>,
    {
        if self.state == ProposalState::Applied || self.state == ProposalState::Cancelled {
            return Err(ApplyError::NotOpen(self.state));
        }
        for handle in group {
            let own = self
                .handles
                .get(&handle.local_id)
                .ok_or(ApplyError::UnknownHandle(handle.local_id))?;
            if own.run_id != self.run_id {
                return Err(ApplyError::WrongRun { expected: self.run_id, actual: own.run_id });
            }
            if own.proposal_id != self.proposal_id {
                return Err(ApplyError::WrongProposal {
                    expected: self.proposal_id,
                    actual: own.proposal_id,
                });
            }
            if own.category != self.category {
                return Err(ApplyError::WrongType { expected: self.category, actual: own.category });
            }
            if self.stale.contains(&handle.local_id) {
                return Err(ApplyError::Stale(handle.local_id));
            }
            if self.locked.contains(&handle.local_id) {
                return Err(ApplyError::Locked(handle.local_id));
            }
        }
        let resolved = commit(group).map_err(ApplyError::Commit)?;
        for r in &resolved {
            self.committed.insert(r.local_id);
        }
        self.state = if self.committed.len() == self.handles.len() {
            ProposalState::Applied
        } else {
            ProposalState::PartiallyApplied
        };
        Ok(ApplyOutcome { resolved })
    }

    /// Call the proposal off. Only an open one can be.
    pub fn cancel(&mut self) -> Result<(), CancelError> {
        if self.state != ProposalState::Open {
            return Err(CancelError::NotCancellable(self.state));
        }
        self.state = ProposalState::Cancelled;
        Ok(())
    }
}
