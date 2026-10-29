# G-S01 — Test

## Test execution
```
$ cargo check -p openbim-desktop
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.98s

$ cargo test -p openbim-desktop --lib
test result: ok. 272 passed; 0 failed; 1 ignored; 0 measured; finished in 169.05s

$ cargo test -p openbim-desktop --lib model_view::tests::the_two_storey
test model_view::tests::the_two_storey_office_is_drawn_saved_reopened_and_exported_through_the_window ... ok
```

## Results
- All 272 existing tests pass (0 failures, 1 pre-existing ignore).
- No regressions from door swing arc + handle code.
- Borrow checker satisfied; no memory safety issues.
