# IFC class map (24 Oct 2026)

Revit lets the user edit a text file that says which Revit category each IFC class becomes. This app now has the same, as
step 3 of that approach (after the import log and the generic carrier).

## The file

Plain text, one row per line, tab or space separated, `#` starts a comment:

```
# IFC class      app category   predefined type (optional)
IFCSTAIR         Generic
IFCSLAB          Generic        LANDING
IFCBUILDINGELEMENTPROXY  Ignore
IFCMEMBER        Column
```

Categories: `Wall`, `Beam`, `Column`, `Slab`, `Footing`, `WallFooting`, `Pipe`, `Duct`, `Generic`, `Ignore`. A row with a
predefined type wins over the row for the class. A class with no row is read as before. A bad row (not an IFC class, unknown
category, a class given twice, too many columns) refuses the whole map and names its line; the import then does not start.
At most 10 000 rows and 1 MB.

## In the app

The Import IFC window has a Class map field with Browse and "Save example..." (a starting file). The log (`<file>.log`) says
which map was used and, for each product the map left out, a line saying the map did.

## What it does

- `Ignore`: the class is left out and counted as "(the class map says to leave it out)".
- `Generic`: kept as a generic model with its class and properties, also for classes the importer reads (a stair, a roof, a
  landing, a covering). A product with no extruded body stays out and is counted.
- A category: the product is read as that element from its extruded body, also for classes the importer did not read (a
  pump as a slab). The element's own rules still apply (a column needs its top on a level), and what fails is counted.
- Rooms, doors and windows are always rooms, doors and windows; a row for them is ignored unless it says `Ignore`, and the
  import says so.
- A file with the app's own records is put back from them; the map is for files read from their geometry.

## Not done

Curved grids, stairs and equipment as real elements, and a map editor inside the app (the file is edited in a text editor).

## Checks

Parser tests (tabs and spaces, comments, predefined type precedence, every bad row names its line, written-out map reads
back). Import tests: Ignore, class to Slab, walls to Generic, a Generic row with no prism body stays counted. Window test:
a bad map refuses before reading and the log says so; a good map is applied. Mutation-checked: Ignore disabled and category
rerouting disabled each fail tests.
