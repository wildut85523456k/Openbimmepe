# Rooms come back from IFC (23 Oct 2026)

Branch `feat/ifc-room-import`, on top of `feat/electrical-exchange`. Completes the round trip of the room export
(`2026-10-23-ifc-rooms-georef-ids.md`).

## What changed
- The reader (`crates/ifc/src/reader.rs`) now reads `IFCSPACE` as a member with its floor outline; before, it only counted it as left out.
- The export writes the room number as its own property (`OpenBIM_Space.Number`) so a reader can tell it from the name
  (the IFC name stays "number name").
- The import (`apps/desktop/src/import.rs`, `plan_rooms`) makes a room of each space: number from that property, the name
  without the number, department from `Usage`. A point inside the outline is found (centroid, else a corner triangle for
  concave outlines).

## Rules, as built
- A room in this app is the closed space inside the walls around a point. A space becomes a room only if, once the file's
  walls are in, they enclose that point. Otherwise it is listed as left out ("IFCSPACE that the walls do not enclose").
- A number another room of that level has is not taken: the room comes in without a number and a note says so.
- A space whose room is already in the model is skipped quietly (importing twice does not double).
- Files with the app's own data bring their rooms through the records as before.
- The room's height and the file's outline are not kept: both come from the walls and levels of the model.

## Tests
`a_space_in_a_file_comes_back_as_a_room_with_its_number_name_and_department` (geometry only and with own data; second
import adds nothing), `a_space_the_walls_do_not_enclose_and_a_number_already_taken_are_said_not_guessed`.

## Limits
- Spaces with a non-extruded shape (a mesh, a swept solid) are left out.
- A foreign space drawn to the wall centrelines still lands in the right room (any inside point works), but its own area is
  not compared with the room the walls make.
