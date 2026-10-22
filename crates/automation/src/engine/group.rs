//! Dependency groups: which pending creations have to be committed together.
//!
//! Two creations are co-committed when one was made against the other, directly
//! or through a chain: a fitting and the segment it sits on, a route and the
//! fittings rule 5 keeps whole. A group is the connected component of handles
//! under that relation.
use super::handle::{LocalId, PendingHandle};
use std::collections::{BTreeMap, BTreeSet};

/// The connected components of `handles` under `depends_on`, each in a stable
/// order (members sorted by their local id).
///
/// `depends_on[a]` lists the local ids `a` was created against. A dependency on
/// an id that is not among `handles` is ignored here; whether it is stale is the
/// engine's call. The result does not depend on the order the dependencies were
/// listed in, because each component is walked breadth-first and then sorted.
pub fn op_group(
    handles: &[PendingHandle],
    depends_on: &BTreeMap<LocalId, Vec<LocalId>>,
) -> Vec<Vec<PendingHandle>> {
    let present: BTreeSet<LocalId> = handles.iter().map(|h| h.local_id).collect();
    let mut adjacency: BTreeMap<LocalId, BTreeSet<LocalId>> = handles
        .iter()
        .map(|h| (h.local_id, BTreeSet::new()))
        .collect();
    for (from, tos) in depends_on {
        if !present.contains(from) {
            continue;
        }
        for to in tos {
            if !present.contains(to) {
                continue;
            }
            adjacency.get_mut(from).expect("present handle").insert(*to);
            adjacency.get_mut(to).expect("present handle").insert(*from);
        }
    }

    let mut seen: BTreeSet<LocalId> = BTreeSet::new();
    let mut groups: Vec<Vec<PendingHandle>> = Vec::new();
    for handle in handles {
        if seen.contains(&handle.local_id) {
            continue;
        }
        let mut component: BTreeSet<LocalId> = BTreeSet::new();
        let mut queue: Vec<LocalId> = vec![handle.local_id];
        while let Some(id) = queue.pop() {
            if !component.insert(id) {
                continue;
            }
            seen.insert(id);
            if let Some(neighbors) = adjacency.get(&id) {
                queue.extend(neighbors.iter().copied());
            }
        }
        let mut members: Vec<PendingHandle> = handles
            .iter()
            .copied()
            .filter(|h| component.contains(&h.local_id))
            .collect();
        members.sort_by_key(|h| h.local_id);
        groups.push(members);
    }
    groups
}
