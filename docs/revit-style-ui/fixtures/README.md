# Documentation fixtures v1

Run from the repository root:

```sh
python docs/revit-style-ui/fixtures/validate-fixtures.py
```

- `replication-v1.json`: source-derived expected outputs for the limited Dynamo-evidence profile, with precise source URLs and explicit unresolved cases. Gated records deliberately have no claimed Dynamo output; the expected diagnostic is OpenBIM's implementation gate.
- `graph-v1.json`: own-schema serialization-only fixture, not a Dynamo graph or runnable package. Contains two same-typed Player inputs with distinct bindings, connector endpoints, per-call guides, levels, managed-output policy, quantities, permissions/trust requirements and pinned Python/client API/lock fields. Artifact digests are synthetic examples; `example.invalid` cannot be an installation source. Its levels + explicit-guide combination is intentionally gated for execution, not silently altered by round-trip.
- `validate-fixtures.py`: stdlib-only documentation checks. It does not execute saved Python code, import dependencies, install artifacts, run Dynamo, validate the entire future schema or exercise model/permission enforcement. Arithmetic reference checks merely verify the expected-output transcription/calculations. Binary64 canonical edge cases remain a production acceptance gate.

Canonical serialized bytes use LF in Git; CRLF checkouts are normalized before byte comparison. Fixtures must remain UTF-8. Unknown/unsupported executable settings must never be treated as permission to run merely because a file parses.

Normative field definitions, source/version scope and unresolved acceptance gates: [04b](../04b-replication-and-serialization.md).
