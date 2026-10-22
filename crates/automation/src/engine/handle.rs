//! Pending handles: the identity of what a proposal produced before any of it is
//! part of the model (`04b` §3.1).
//!
//! A handle is only meaningful inside its own run and proposal, and only for the
//! kind of element it was created as. Nothing here touches a document: the
//! engine validates handles and the caller commits them.
use openbim_document::Category;
use uuid::Uuid;

/// One evaluation run. A UUIDv7, so ids made on the same clock are monotonic:
/// they sort in the order the runs happened and never collide across machines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RunId(pub Uuid);

impl RunId {
    /// A fresh run id, time-ordered.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for RunId {
    fn default() -> Self {
        Self::new()
    }
}

/// Which proposal of a run a handle belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProposalId(pub u64);

/// An id that means something only inside one proposal: the proposal's own
/// numbering, before any of it is in the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalId(pub u64);

/// A thing a proposal created that is not yet part of the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingHandle {
    pub run_id: RunId,
    pub proposal_id: ProposalId,
    pub local_id: LocalId,
    /// What the creation produces. A handle is refused against a proposal that
    /// creates something else, so a duct creation cannot be applied as a pipe.
    pub category: Category,
}

impl PendingHandle {
    pub fn new(
        run_id: RunId,
        proposal_id: ProposalId,
        local_id: LocalId,
        category: Category,
    ) -> Self {
        Self { run_id, proposal_id, local_id, category }
    }
}
