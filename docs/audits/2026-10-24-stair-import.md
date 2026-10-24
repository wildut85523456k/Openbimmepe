# Stair import from other programs (2026-10-24)

Before, every `IFCSTAIR` of a file from another program was left out ("not read yet"). A straight run of steps is now
read as a Stair element.

## What is read
An `IFCSTAIR` whose body is an extruded profile (the way this app writes one) standing in a vertical plane, extruded
horizontally across the width, closed along its foot, rising by equal steps:
- width = the extrusion depth; foot of the first riser at the middle of the width; run along the profile;
- the direction of climb is the end where the highest step is; the foot is the other end;
- n steps in the profile are n + 1 risers (the last riser leads to the floor above), riser = rise of the profile / n,
  tread = run / n, rise = (n + 1) risers;
- level: the storey the stair is contained in, else the level at or below the foot; base offset = foot above that level.
It is a plain straight stair, like one placed in the app (`Linear` geometry, `count` = risers).

## What is refused (counted as left out, with the reason in the import log)
- a body that is not an extrusion across the width, a profile not in one vertical plane, fewer than two steps in the body
  (a single tread has no direction of climb), steps of unequal height, figures outside a stair's range;
- `Pset_StairCommon.NumberOfRiser` that is not the steps of the body plus one: the file and its geometry disagree, and
  a wrong stair is worse than none.

## Not done
- Stairs of other shapes in the file: flights (`IFCSTAIRFLIGHT`) with their landings, breps, sweeps. The two runs of an
  L or U stair written by this app come back as two separate straight stairs (the landing slab is still left out).
- Railings of a stair.
- A body whose top step is at the full rise (no last riser to the floor above) is refused when `NumberOfRiser` says so.
