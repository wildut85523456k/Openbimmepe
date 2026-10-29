# G-S01 — Validate

## Validation criteria
1. Door swing arc renders in plan view (V4 visual review): ✅ Validated
2. Move handle appears for selected hosted opening: ✅ Validated
3. Width handles appear for selected hosted opening: ✅ Validated
4. Height handle appears in 3D view: ✅ Validated
5. No regressions in existing tests: ✅ Validated (272/272 pass)
6. Borrow checker satisfied: ✅ Validated (cargo check clean)

## Validation method
- `cargo check -p openbim-desktop` — clean compilation
- `cargo test -p openbim-desktop --lib` — 272 passed, 0 failed, 1 ignored
- `the_two_storey_office` integration test — pass

## Note on Windows
All testing done on Windows (as required — "Linux results alone never close
a gap that names Windows or macOS").
