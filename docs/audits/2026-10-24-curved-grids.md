# Curved grid lines (2026-10-24)

Revit grids can be arcs; the app's grids were straight only, so an arc axis in a foreign IFC file was flattened to a
chord. A grid element now carries a **bulge**.

## Model
- `Element.bulge` (DXF meaning): `tan(sweep/4)` of the arc from start to end, positive counter-clockwise, `|bulge| <= 1`
  (at most a half circle). 0 is a straight line. Only a Grid may have a bulge. `Command::SetBulge` changes it (undoable).
- `grid_arc` gives the arc (centre, radius, sweep), a polyline for drawing (steps of at most 5 degrees, exact ends) and
  the bulge back from a centre and a sense.
- Saved as `bg=` on the element record; a file without it is straight. Mirroring negates the bulge; moving and rotating keep it.
- `place_arc_grid` exists for code and tests; there is no drawing tool for arcs yet.

## Views
Plan views, sheets and plan export draw the arc as a polyline. Bubbles stay at the ends. The 3D datum and the selection
hit-test keep the chord. Properties shows radius, sweep and length (read only).

## IFC
- Out: `IFCTRIMMEDCURVE` over an `IFCCIRCLE` (`IFCAXIS2PLACEMENT2D`), trimmed by the end points, sense `.T.`/`.F.` for the
  direction; all three schemas. Straight lines stay `IFCPOLYLINE`.
- In: trimmed circles trimmed by point or by parameter (degrees, the IFC default). An arc over half a circle cannot be held
  by a bulge: it is kept as a straight line and the import log says so. Import skips a line the model already has (same
  name, ends and bulge).

## Not done
Drawing tool for arcs, major arcs, other curve kinds (B-spline), plane-angle units other than degrees for parameter trims.
