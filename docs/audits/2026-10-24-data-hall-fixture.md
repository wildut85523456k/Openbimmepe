# Data hall fixture and the MEP chain (MEP-004), 24 Oct 2026

`apps/desktop/src/data_hall.rs` (test only) generates the data hall the MEP chain is tested on: one storey, 30 x 18 m,
four walls, a floor slab, a ceiling at 3.2 m, one room (D01 Data hall), two air handling units on the north wall and
eighteen square ceiling diffusers in three rows. Two supply systems, SA-1 and SA-2, are made as Revit's Create Systems >
Duct makes them: each has one air handler (its base) and the nine diffusers of its half of the hall. Nothing is hand-modelled.

## Compared with the Revit approach (mep-part2 sections 5, 7, 8 and tutorial 3)
- Terminals sit on the ceiling and the duct meets the connector on the neck from above: the fixture does that (face at the
  ceiling, connector on top of the neck, facing up). A connector facing up can be reached by another section of duct
  (what is built) or by a flexible duct (not built).
- Generate Layout takes Conversion Settings, the elevation of the main and branches. Built: the main runs above the highest
  connector by 0.55 m (the void above the ceiling) unless an elevation is given in code; the window has no field for it yet.
  Main and branch share one elevation; Revit has two.
- The model holds no vertical run (a run needs a length in plan). So the air handler, which leaves sideways at 0.7 m, rises
  to the main on a 45 degree section; a diffuser (or a sprinkler head: same shape, pipe instead of duct) is reached by a
  branch at the elevation of the main and a last 45 degree section down to its connector. A branch to a column of
  connectors jogs along the trunk so the tees do not meet. A round diffuser neck is joined by a square duct of the same
  area, since the model holds rectangular ducts only.

## The chain, as tested (`the_supply_chain_runs_from_the_air_handler_to_the_diffusers_and_into_the_ifc_file`)
1. Generate Layout for SA-1 with a 400 x 300 trunk: a network at 3.90 m, all 10 connectors reached.
2. Finish Layout: 38 duct sections joined to the 10 connectors, one undo step.
3. End Points: the air handler is the source, the nine diffusers take their flow from their connectors.
4. Compute on SA-1: 0.45 m3/s over nine terminals, none unreached, a critical path.
5. Apply sizes: the branches take smaller sizes than the trunk.
6. IFC: `IfcDistributionSystem` "SA-1" (ventilation) with the air handler assigned to it, 38 duct segments, 18 air
   terminals, a port joined for every connector.
SA-2 lays out as well.

## Not as in Revit
- No flexible duct yet (Revit: Flex Duct Type and Maximum Flex Length in the conversion settings); a 45 degree rigid
  section stands in. No Perimeter or Intersections solution types. The elevation is not in the window.
- The trunk size is typed in the window (here 400 x 300): by default it is the largest connector (the air handler's
  800 x 500), too big for tees 2.4 m apart, which then do not fit; Compute resizes afterwards.
- Terminals are not hosted by the ceiling (Place on Face) and there are no spaces with airflow, return grilles or return
  system; those are separate rows of the gap list.

Verified: the layout and the chain tests pass; two mutations (the elevated layout switched off; the tee stagger removed)
each make the chain test fail.
