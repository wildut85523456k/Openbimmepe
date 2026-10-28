# G-M02 — Test

## Closing test

```sh
cargo test --workspace  # 272 passed, 0 failed, 1 ignored — exit 0
```

## Clippy

```sh
cargo clippy --workspace  # exit 0, no warnings
```

## Before fix (base SHA had G-M02 failure)

```
test result: FAILED. 271 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out
left: ("", "Room 1")
right: ("101", "Office")
```

## After fix (candidate SHA `d8cd1c3`)

```
test result: ok. 272 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

## Verdict

**PASS** — failing → passing.
