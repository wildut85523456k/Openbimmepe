# 0001 - Storage model: text file for saving, operation log for keeping work

Status: proposed for the owner, implemented for autosave and recovery (23 Oct 2026). Closes open decision 1 of
`docs/gap-remediation-plan-2026-10-21.md`, section 6.

## Question

The agreed model was "operation log + snapshots". The app writes a full text snapshot on Save and `crates/oplog` was
not used. Is the log still wanted, and if so, what is in it?

## Findings

- The document has about 25 command kinds and holds its MEP network as one block (`SetMep`). Writing each command to
  a log would need a binary form for every command and for that block, kept in step with the file format. Two
  descriptions of the model would drift.
- `crates/oplog` is generic: elements with numbered properties, a CRC per record, torn-tail detection, snapshots,
  merge by element and property. It needs no knowledge of the document.
- Undo in the document keeps a full copy of the model per step. A log cannot replace that cheaply; they are separate
  questions.
- The `.obim` text is one record per line and already the definition of the format.

## Decision

1. **The text file stays** the thing the user saves, opens, exchanges and diffs. It is written whole and renamed into
   place (unchanged).
2. **The log's state is the file's records.** One log element per record: property 0 is the record line, property 1 its
   place in the file. A record with an `id=` field is identified by tag and id, so an edited element is one `Set`.
   Any other record is identified by its text, so a change is a `Delete` and a `Create`.
3. **A transaction is the difference between two states of the records**, so the log never needs to know what a command
   is. The format is described in one place only, `native.rs`.
4. **The journal is the recovery store.** `<file>.journal` (or a file in the temporary folder for an unnamed model) holds
   a base snapshot, then appended transactions. It is written while the model has unsaved changes, every N minutes as the user chooses in Options (default 1, 0 = off). When it
   reaches 500 transactions or 8 MB it is folded into a new base. A normal Save deletes it. It never replaces the file.
5. **Reading tolerates a crash.** A torn last record is dropped. A damaged record stops the replay there and the
   recovered work up to it is offered, with a message saying so.

## What this does not decide

- **Undo and redo** keep their own history. Deriving them from the log is possible later and is not needed now.
- **Multi-user.** Records are keyed by element, so `oplog::merge` can join two journals by element and property. Two
  authors editing the same record conflict, as `merge` reports. Author numbers are not assigned yet (always 0).
- **Records without an id** (links, end points, settings) merge as add and remove, not by field.
- **Order.** A record inserted in the middle of the file changes the place of those after it, so the transaction holds
  those places too. Records are mostly appended, so this is small in practice.

## Consequences

- Cost of a transaction is the size of the changed records. A test checks that three beams add well under a quarter of
  the model size each.
- Crash safety no longer depends on rewriting the whole model during editing.
- Nothing in the file format changed.

## If the owner prefers otherwise

The journal is confined to `apps/desktop/src/journal.rs` and the autosave functions in `file.rs`. Removing it returns to
the snapshot copy of the first version of B20 (`git show 91eb050`). Making it the primary store instead (opening a file
by replaying) is a further step that needs the owner's decision on file extension and exchange format.
