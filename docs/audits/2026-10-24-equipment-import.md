# Equipment import from other programs (2026-10-24)

Before, a pump, fan, air terminal or unitary unit in a foreign file was either a generic model (when its body is an
extruded profile) or left out (any other body, which is what equipment nearly always has). The reader now also lists
products of the equipment classes (IFCAIRTERMINAL, FAN, DAMPER, PUMP, BOILER, CHILLER, COIL, UNITARYEQUIPMENT, TANK, VALVE,
SANITARYTERMINAL, LIGHTFIXTURE, ELECTRICDISTRIBUTIONBOARD, OUTLET, SWITCHINGDEVICE, FIRESUPPRESSIONTERMINAL, FURNITURE)
whatever their body, with their place and turn, and the importer makes a **library family instance** of those it can.

## Which family
1. A row of the class map: `IFCPUMP<TAB>family:<name of a library family>[<TAB>predefined type]` (see
   `docs/audits/2026-10-24-ifc-class-map.md`). A name that is not in the library keeps the product as it was before
   (generic model if it has an extruded body, else left out).
2. Else the family the file names itself: the product's type carries `OpenBIM_Family.Family`, as this app writes it
   even without its own records. Only if that library family has the product's IFC class (a mismatch is not guessed).
Doors and windows are never made this way (they have their own path, in walls).

## What is read
Place (x, y), height above the level the product is in, turn from its x axis, the type when the family has one of that
name, and the instance values (`OpenBIM_Instance`) that are inputs of the family. A product the family does not accept
stays left out and the log says so. A family of the same kind in the same place (1 cm, 5 mm in height) is not made
twice; with element numbers, the number decides as for other elements. A file with the app's own records restores its
families from the records, as before; the equipment of the file is not read again on top.

## Not done
- Ports and the pipes, ducts and cables that join the equipment: another program's connections are not read.
- Size from the product's own body: a family comes at its defaults and the instance values of the file; a pump of
  another make does not become a pump family (the library has none: it has a fan coil unit, an air handling unit and a
  ceiling diffuser). More families come with the list of MEP families still to be decided.
- Hosting: a family made for a ceiling is placed free at its height, not hosted.
