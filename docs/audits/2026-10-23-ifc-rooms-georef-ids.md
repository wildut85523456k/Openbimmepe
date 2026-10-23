# IFC export: rooms, georeferencing and stable ids (23 Oct 2026)

Branch `feat/ifc-rooms-georef-ids`, on top of `feat/shell-b2-b25`. Closes the "not done" list of
`2026-10-23-ifc-honesty-bundle.md`, except B18's remainder.

| Gap | What changed | Test |
|---|---|---|
| Rooms did not leave | Each room is an `IfcSpace` (name = number and name, use = department, floor outline, holes, height to the next level, 2.7 m on the top level) with its quantities | `a_room_leaves_as_a_space_with_its_number_name_and_department` |
| Site and map did not leave | A Georeferencing window (map name, datums, projection, zone, survey point shared coordinates and place in the model, site elevation, file origin). Saved in the file as a `U|kind=georef` line. With a site and a map name the file has `IfcProjectedCRS`, `IfcMapConversion`, true north and the site position; without a name it has none and the export says what to do | `the_site_leaves_with_its_map_once_the_map_has_a_name`, `the_map_survives_the_file_and_needs_a_name_and_a_site` |
| Ids of relationships and property sets came from a counter | They come from what the thing is: its text, with each reference replaced by the id of what it refers to, and for a property set, what refers to it. Adding an element before another no longer changes the other's relationships | `adding_a_room_before_another_leaves_the_ids_of_the_other_alone` (IFC4, 4x3, 2x3), `every_id_in_a_file_is_different_and_exports_repeat_exactly`, `two_property_sets_that_are_the_same_in_everything_are_still_told_apart` |

## Limits

- The map is a name the author types. No projection arithmetic is done, as in the georef module's own description.
- Georeferencing is per model and needs the site (latitude, longitude, angle) already set in the 3D view.
- Room height is the distance to the next level, not the room's own limit offsets.
- Two property sets that are the same in everything, attached to things that are also the same, are told apart by their
  order; that is the one case where an id can move when something is added before it.
- Rooms are written but not yet read back by the IFC import.
- The georeferencing window and the Options button were not looked at in the running window.
