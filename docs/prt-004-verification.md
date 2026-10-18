# PRT-004 candidate verification — 2026-10-18

Status: implemented and verified at the sheets-crate level; see the implementation PR for publication/merge status.
This follows PRT-001 on the sheets source baseline
18520949edece94c8db30f764378ba818d6433bf. PR #45 remains documentation-only.

## Decision

Retain R12 export and replace six-base-colour matching with nearest fixed ACI matching.
Reference RGB data is pinned to ezdxf 1.4.4 DXF_DEFAULT_COLORS (AutoCAD 2020 modelspace
background RGB 33,40,48). This is a deterministic reference, not a universal RGB guarantee.
See https://ezdxf.readthedocs.io/en/stable/colors.html#default-palettes and
https://github.com/mozman/ezdxf/blob/v1.4.4/src/ezdxf/colors.py.

Search indices 1-255 using squared RGB distance and the lowest index for ties.
Index 0 (BYBLOCK) is excluded; index 256 (BYLAYER) is never produced.
Black retains adaptive foreground 7. Other RGB values exclude index 7 to avoid exporting
white as adaptive black on a light background. Other grayscale values use nearest matching
instead of the old two-gray threshold. PDF rendering is unchanged.

## Evidence

Linux x86_64; rustc 1.99.1; existing upstream dependency lockfile, no Cargo dependency change.

- New team-colour test before the implementation: FAIL as expected, olive produced ACI 1.
- Candidate release test suite: PASS, 63 tests.
- Clippy --release --all-targets -- -D warnings: PASS, exit 0.
- Independent Python checker: PASS, 1005 distinct source RGB cases drawn from all reference
  palette RGB values, an RGB grid, seeded random values and the three documented team colours.
  Checks actual exported entity and layer colours, saves/reopens with ezdxf, then checks again.
- Team olive (122,122,0): ACI 54; purple (112,0,223): ACI 190;
  dark magenta (183,0,183): ACI 212.
- The checker also verifies all 1005 source PDF stroke colours remain unchanged within
  the PDF writer's three-decimal precision.
- PRT-001 regression remains in the passing release suite.

## Limits

ACI quantization is lossy and palette/background dependent. True-colour DXF is deferred;
it would require a format/consumer compatibility decision. Full workspace integration,
desktop interaction, receiving AutoCAD visual review and independent code review have not run.
This report covers PRT-004 only; the later PRT-003 stage is documented separately.
PRT-002/005/006/007 and DOC-PURGE remain outside this implementation.
