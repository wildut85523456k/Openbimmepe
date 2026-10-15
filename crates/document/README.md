# openbim-document

First in-memory document prototype for Sprint 1. Supplies stable element IDs,
typed wall/duct/pipe/equipment records, atomic create/resize/elevation/delete
transactions, optimistic document revision checks and local undo.

It is **not** a production BIM document. Still missing: serialized persistence,
semantic host/connectivity relationships, family type/instance parameters, IFC
import/export bridge, distributed ID allocation, semantic dependencies, bounded
history, central server, replay/recovery, and multi-user locking.

Run tests: `cargo test -p openbim-document`.

The existing `openbim-oplog` is more sophisticated for binary logs and merge
experiments. Before production, reconcile the two transaction models rather
than maintaining incompatible command paths.
