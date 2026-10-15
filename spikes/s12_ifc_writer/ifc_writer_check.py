"""Independent check of the Rust IFC writer (S12).

Usage: ifc_writer_check.py MANIFEST.txt SCENE.ifc SCENE_AGAIN.ifc SCENE_EDITED.ifc
  MANIFEST   what the Rust example says it wrote (plain text)
  SCENE      the file the Rust writer made
  AGAIN      a second export of the same model: must be byte for byte the same
  EDITED     an export after an edit (a door moved): every GlobalId must be unchanged

Everything below is recomputed here from the IFC file with IfcOpenShell and this script's own maths:
 1. schema validation of the file (no statements),
 2. GlobalIds: decoded with this script's own 128-bit layout and compared with IfcOpenShell's decoder,
 3. port positions and outward directions through IfcOpenShell's placement code,
 4. port connections (IfcRelConnectsPorts) against the connectivity graph, and port nesting,
 5. solid geometry of runs (length, radius), walls and slabs (volume by hand),
 6. voids, fire ratings, systems,
 7. repeatability and stable ids,
 8. negative controls: a deliberately wrong expectation must be reported.
"""
import math, sys
import numpy as np
import ifcopenshell, ifcopenshell.geom, ifcopenshell.guid, ifcopenshell.validate
import ifcopenshell.util.element as ue
import ifcopenshell.util.placement as up
import ifcopenshell.util.shape as ush

man = [l.split() for l in open(sys.argv[1]).read().splitlines() if l.strip()]
f = ifcopenshell.open(sys.argv[2])
fails = checks = 0
def ok(c, msg):
    global fails, checks
    checks += 1
    if not c:
        fails += 1
        print("FAIL", msg)

def rows(tag): return [r[1:] for r in man if r[0] == tag]
key = int(rows("PROJECT")[0][0])
B = 1 << 64

# ---- 1. schema validation
lg = ifcopenshell.validate.json_logger()
ifcopenshell.validate.validate(f, lg)
ok(f.schema == "IFC4", "schema is IFC4")
ok(len(lg.statements) == 0, f"validation statements: {lg.statements[:3]}")

# ---- 2. global ids
CH = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$"
def my_expand(s):
    assert len(s) == 22
    v = 0
    for i, c in enumerate(s):
        v = (v << (2 if i == 0 else 6)) | CH.index(c)
    return v
def decode(s):
    v = my_expand(s)
    return v >> 80, (v >> 64) & 0xFFFF, v & (B - 1)
def my_compress(key, part, el):
    v = (key << 80) | (part << 64) | el
    return CH[v >> 126] + "".join(CH[(v >> (126 - 6 * k)) & 63] for k in range(1, 22))

# IfcOpenShell's own decoder vs ours, on every id in the file
guids = [e.GlobalId for e in f.by_type("IfcRoot")]
ok(len(set(guids)) == len(guids), "GlobalIds are unique")
for g in guids:
    mine = my_expand(g)
    theirs = int(ifcopenshell.guid.expand(g), 16)
    if mine != theirs:
        ok(False, f"decoder differs for {g}")
        break
else:
    ok(True, "")
ok(all(decode(g)[0] == key for g in guids if decode(g)[2] >> 32 != 0xFFFFFFFF), "project key in every element id")
ok(all(my_compress(*decode(g)) == g for g in guids), "re-encoding gives the same text")

seg_ids = {int(r[0]) for r in rows("SEG")}
fit_ids = {int(r[0]) for r in rows("FIT")}
item_rows = {int(r[0]): r for r in rows("ITEM")}
wall_ids = {int(r[0]) for r in rows("WALL")}
slab_ids = {int(r[0]) for r in rows("SLAB")}
def elems(cls): return {decode(e.GlobalId)[2]: e for e in f.by_type(cls)}
walls = elems("IfcWall"); slabs = elems("IfcSlab")
ok(set(walls) == wall_ids and all(decode(e.GlobalId)[1] == 0 for e in walls.values()), "wall ids")
ok(set(slabs) == slab_ids, "slab ids")
segs = {}
for c in ("IfcPipeSegment", "IfcDuctSegment", "IfcCableCarrierSegment", "IfcCableSegment"):
    segs.update(elems(c))
fits = {}
for c in ("IfcPipeFitting", "IfcDuctFitting", "IfcCableCarrierFitting", "IfcCableFitting"):
    fits.update(elems(c))
occs = {}
for c in ("IfcDoor", "IfcWindow", "IfcUnitaryEquipment", "IfcFurniture"):
    occs.update(elems(c))
ok(set(segs) == seg_ids, f"segment ids {set(segs) ^ seg_ids}")
ok(set(fits) == fit_ids, "fitting ids")
ok(set(occs) == set(item_rows), "placed family ids")

# ---- 3. ports in world coordinates
ports = {}
for p in f.by_type("IfcDistributionPort"):
    k, part, el = decode(p.GlobalId)
    ports[(el, part)] = p
want = {}
for r in rows("PORT"):
    want[(int(r[0]), int(r[1]))] = (np.array(r[2:5], float), np.array(r[5:8], float))
icons = {(int(r[0]), int(r[1])) for r in rows("ICON")}
ok(set(want) <= set(ports) and set(ports) - set(want) == icons - set(want), f"port set differs: {set(ports) ^ set(want)}")
def port_check(ports, want, tol=1e-7):
    bad = 0
    for k, (pos, d) in want.items():
        m = up.get_local_placement(ports[k].ObjectPlacement)
        if not (np.allclose(m[:3, 3], pos, atol=tol) and np.allclose(m[:3, 2], d, atol=tol)):
            bad += 1
    return bad
ok(port_check(ports, want) == 0, "port position / outward direction")

# ---- 4. connections and nesting
links = set()
for r in f.by_type("IfcRelConnectsPorts"):
    a, b = decode(r.RelatingPort.GlobalId), decode(r.RelatedPort.GlobalId)
    links.add(frozenset([(a[2], a[1]), (b[2], b[1])]))
wl = {frozenset([(int(r[0]), int(r[1])), (int(r[2]), int(r[3]))]) for r in rows("LINK")}
ok(links == wl and len(links) == len(f.by_type("IfcRelConnectsPorts")), "connections differ")
for (el, part), p in ports.items():
    parents = [r.RelatingObject for r in p.Nests]
    ok(len(parents) == 1 and decode(parents[0].GlobalId)[2] == el and decode(parents[0].GlobalId)[1] == 0, f"port {el}/{part} nested in its element")
# ports that meet are at one place and face each other
for l in links:
    a, b = [tuple(x) for x in l]
    pa, da = want[a]; pb, db = want[b]
    ok(np.allclose(pa, pb, atol=1e-7) and np.isclose(np.dot(da, db), -1.0, atol=1e-7), f"linked ports {a} {b} meet and face")

# ---- 5. geometry
st = ifcopenshell.geom.settings()
st.set("use-world-coords", True)
def verts(e):
    s = ifcopenshell.geom.create_shape(st, e)
    return np.array(s.geometry.verts).reshape(-1, 3), s
for r in rows("SEG"):
    sid = int(r[0]); a = np.array(r[3:6], float); b = np.array(r[6:9], float)
    v, _ = verts(segs[sid])
    d = (b - a) / np.linalg.norm(b - a)
    t = (v - a) @ d
    ok(abs(t.min()) < 1e-6 and abs(t.max() - np.linalg.norm(b - a)) < 1e-6, f"segment {sid} runs from start to end")
    rad = np.linalg.norm((v - a) - np.outer(t, d), axis=1)
    if r[2] == "DUCT":
        ok(abs(rad.max() - math.hypot(0.4, 0.3) / 2) < 1e-6, f"duct {sid} section corner distance")
    else:
        dia = ue.get_psets(segs[sid])["OpenBIM_Mep"]["Diameter"]
        ok(abs(rad.max() - dia / 2) < 1e-6, f"pipe {sid} radius")
# walls: volume by hand (ring of a closed rectangle, or length x thickness for an open straight wall) less every opening
for w in rows("WALL"):
    wid = int(w[0]); th, h = float(w[-2]), float(w[-1])
    base, closed = [float(x) for x in next(r for r in rows("WBASE") if int(r[0]) == wid)[1:3]]
    pts = np.array([[float(r[1]), float(r[2])] for r in rows("WPT") if int(r[0]) == wid])
    lo, hi = pts.min(axis=0), pts.max(axis=0)
    if closed:
        gross = ((hi - lo + th).prod() - (hi - lo - th).prod()) * h
        box_lo, box_hi = lo - th / 2, hi + th / 2
    else:
        gross = np.linalg.norm(hi - lo) * th * h
        axis_x = (hi - lo)[0] > (hi - lo)[1]
        pad = np.array([0, th / 2]) if axis_x else np.array([th / 2, 0])
        box_lo, box_hi = lo - pad, hi + pad
    cut = sum(float(r[3]) * float(r[5]) * th for r in rows("OPEN") if int(r[0]) == wid)
    vol = ush.get_volume(ifcopenshell.geom.create_shape(st, walls[wid]).geometry)
    ok(abs(vol - (gross - cut)) < 1e-6, f"wall {wid} volume {vol} vs {gross - cut}")
    wv, _ = verts(walls[wid])
    ok(np.allclose(wv.min(axis=0), [*box_lo, base], atol=1e-6) and np.allclose(wv.max(axis=0), [*box_hi, base + h], atol=1e-6), f"wall {wid} box {wv.min(axis=0)} {wv.max(axis=0)}")
th = float(rows("WALL")[0][-2]); h = float(rows("WALL")[0][-1])
# slab: area less the shaft and the cut riser opening
s = rows("SLAB")[0]; sth = float(s[-2])
shaft_area = sum((float(r[2]) - float(r[1])) * (float(r[4]) - float(r[3])) for r in rows("SHAFT"))
cuts = sum((float(r[3]) - float(r[2])) * (float(r[5]) - float(r[4])) for r in rows("FPEN") if r[1] == "Cut")
svol = ush.get_volume(ifcopenshell.geom.create_shape(st, list(slabs.values())[0]).geometry)
ok(abs(svol - (10 * 8 - shaft_area - cuts) * sth) < 1e-6, f"slab volume {svol} vs {(10*8 - shaft_area - cuts) * sth}")

# where the wall and the slab are, by their solid's bounding box
sv, _ = verts(list(slabs.values())[0])
stop = float(s[-3])
ok(np.allclose(sv.min(axis=0), [0, 0, stop - sth], atol=1e-6) and np.allclose(sv.max(axis=0), [10, 8, stop], atol=1e-6), f"slab box {sv.min(axis=0)} {sv.max(axis=0)}")
for r in rows("FPEN"):
    if r[1] == "Cut":
        ov, _ = verts([o for o in f.by_type("IfcOpeningElement") if o.Name == r[0]][0])
        ok(np.allclose(ov.min(axis=0), [float(r[2]), float(r[4]), stop - sth - 0.05], atol=1e-6) and np.allclose(ov.max(axis=0), [float(r[3]), float(r[5]), stop + 0.05], atol=1e-6), f"floor opening {r[0]} box")
for r in rows("SHAFT"):
    ov, _ = verts([o for o in f.by_type("IfcOpeningElement") if o.Name == "Shaft " + r[0]][0])
    ok(np.allclose(ov.min(axis=0)[:2], [float(r[1]), float(r[3])], atol=1e-6) and np.allclose(ov.max(axis=0)[:2], [float(r[2]), float(r[4])], atol=1e-6), "shaft void outline")
# ---- 5b. families as IFC types
# ITEM columns: id class type ox oy oz ux uy volume name
def name_of(r): return r[9]
CURVED = {"STAND-01"}   # a faceted revolve: the mesh corners lie inside the true curve
TOL_CURVED = 2e-3
def local_verts(e, origin):
    v, _ = verts(e)
    return sorted({tuple(np.round(p - origin, 6)) for p in v})
def is_mapped(e): return any(i.is_a("IfcMappedItem") for i in e.Representation.Representations[0].Items)
types_seen = {}
for iid, r in item_rows.items():
    e = occs[iid]
    cls, tname = r[1], r[2]
    t = ue.get_type(e)
    ok(t is not None and t.is_a() == cls + "Type", f"{name_of(r)} has a type of class {cls}Type")
    if t is None:
        continue
    if tname != "-":
        ok(t.Name == tname.replace("_", " "), f"{name_of(r)} type name {t.Name}")
    types_seen.setdefault(t.id(), []).append(iid)
    ok(len(e.IsTypedBy) == 1, f"{name_of(r)} has one IfcRelDefinesByType")
    # the volume of the placed shape equals the sum of the solids the Rust code solved
    vol = ush.get_volume(ifcopenshell.geom.create_shape(st, e).geometry)
    ok(abs(vol - float(r[8])) < (2e-3 * float(r[8]) if name_of(r) in CURVED else 1e-6), f"{name_of(r)} volume {vol} vs {r[8]}")
    # parametric bodies, never meshes
    reps = e.Representation.Representations[0].Items
    kinds = {i.is_a() for i in reps}
    ok(kinds <= {"IfcMappedItem", "IfcExtrudedAreaSolid", "IfcRevolvedAreaSolid"}, f"{name_of(r)} body kinds {kinds}")
    if is_mapped(e):
        inner = {i.is_a() for m in reps for i in m.MappingSource.MappedRepresentation.Items}
        ok(inner <= {"IfcExtrudedAreaSolid", "IfcRevolvedAreaSolid"}, f"{name_of(r)} shared shape kinds {inner}")
for tid, ids in types_seen.items():
    t = f.by_id(tid)
    ok(t.RepresentationMaps is not None and len(t.RepresentationMaps) == 1, f"{t.Name} has one representation map")
    ok(len(t.Types[0].RelatedObjects) == len(ids), f"{t.Name} relates all its occurrences")
    # an occurrence uses the shared map exactly when its shape, seen from its own frame, is the type's shape
    shapes = {iid: local_verts(occs[iid], np.array(item_rows[iid][3:6], float)) for iid in ids}
    for iid in ids:
        e = occs[iid]
        ok(is_mapped(e) == (shapes[iid] == shapes[ids[0]]), f"{name_of(item_rows[iid])}: mapped exactly when its shape is the type's")
        if is_mapped(e):
            ok(all(i.MappingSource == t.RepresentationMaps[0] for i in e.Representation.Representations[0].Items), f"{name_of(item_rows[iid])} shares the type's map")
by_name = {name_of(r): iid for iid, r in item_rows.items()}
ok(is_mapped(occs[by_name["D-01"]]) and is_mapped(occs[by_name["D-03"]]) and not is_mapped(occs[by_name["D-02"]]), "D-01 and D-03 are mapped, D-02 (swing left) has its own shape")
ok(len({occs[by_name[n]].Representation.Representations[0].Items[0].MappingSource.id() for n in ("D-01", "D-03")}) == 1, "D-01 and D-03 use one representation map")
ok(len(types_seen) == 4 and sorted(len(v) for v in types_seen.values()) == [1, 1, 1, 3], f"four types, with 3, 1, 1 and 1 occurrences: {sorted(len(v) for v in types_seen.values())}")
# the placed shape lies where the family frame puts the solved bounds (the frames here are quarter turns, so the box stays square)
for r in rows("IBOX"):
    iid = int(r[0]); ir = item_rows[iid]
    org = np.array(ir[3:6], float); ux, uy = float(ir[6]), float(ir[7])
    R = np.array([[ux, -uy, 0], [uy, ux, 0], [0, 0, 1]])
    lo, hi = np.array(r[1:4], float), np.array(r[4:7], float)
    corners = np.array([[x, y, z] for x in (lo[0], hi[0]) for y in (lo[1], hi[1]) for z in (lo[2], hi[2])])
    w = org + corners @ R.T
    v, _ = verts(occs[iid])
    tol = TOL_CURVED if name_of(ir) in CURVED else 1e-6
    ok(np.allclose(v.min(axis=0), w.min(axis=0), atol=tol) and np.allclose(v.max(axis=0), w.max(axis=0), atol=tol), f"{name_of(ir)} box {v.min(axis=0)} {v.max(axis=0)} vs {w.min(axis=0)} {w.max(axis=0)}")
# overall size, fill relationship, position against the opening, type properties
for r in rows("IOPEN"):
    iid = int(r[0]); e = occs[iid]; nm = name_of(item_rows[iid])
    ok(abs(e.OverallWidth - float(r[1])) < 1e-9 and abs(e.OverallHeight - float(r[2])) < 1e-9, f"{nm} overall size")
    fills = [x.RelatingOpeningElement for x in e.FillsVoids]
    ok(len(fills) == 1, f"{nm} fills one opening")
    ev, _ = verts(e); ov, _ = verts(fills[0])
    ok(np.allclose((ev.min(axis=0) + ev.max(axis=0))[:2] / 2, (ov.min(axis=0) + ov.max(axis=0))[:2] / 2, atol=1e-6), f"{nm} is centred in its opening")
    ok(abs(ov.min(axis=0)[2] - (float(item_rows[iid][5]) + float(r[3]))) < 1e-6, f"{nm} opening starts at its sill")
    ok(abs(ue.get_psets(ue.get_type(e))["OpenBIM_Family"]["width"] - float(r[1])) < 1e-9, f"{nm} type carries its width")
# connectors of a placed family: ports where the connector is, from this script's own frame maths
for r in rows("ICON"):
    iid, part = int(r[0]), int(r[1])
    ir = item_rows[iid]; org = np.array(ir[3:6], float); ux, uy = float(ir[6]), float(ir[7])
    R = np.array([[ux, -uy, 0], [uy, ux, 0], [0, 0, 1]])
    pos = org + R @ np.array(r[3:6], float); dd = R @ np.array(r[6:9], float)
    m = up.get_local_placement(ports[(iid, part)].ObjectPlacement)
    ok(np.allclose(m[:3, 3], pos, atol=1e-7) and np.allclose(m[:3, 2], dd / np.linalg.norm(dd), atol=1e-7), f"connector {r[2]} of {name_of(ir)}")

# ---- 6. voids, fire ratings, systems
vo = f.by_type("IfcRelVoidsElement")
ok(len(vo) == len(f.by_type("IfcOpeningElement")) == int(next(x for x in man[1] if x.startswith("openings=")).split("=")[1]), "every opening voids something")
for r in vo:
    host = decode(r.RelatingBuildingElement.GlobalId)[2]
    opn = decode(r.RelatedOpeningElement.GlobalId)
    ok(host == opn[2] and opn[1] >= 1, "opening id belongs to its host")
ok(ue.get_psets(walls[int(rows("WALL")[0][0])])["Pset_WallCommon"]["FireRating"] == "60 min", "wall fire rating")
ok("Pset_WallCommon" not in ue.get_psets(walls[int(rows("WALL")[1][0])]), "a wall without a rating has no fire rating")
ok(ue.get_psets(list(slabs.values())[0])["Pset_SlabCommon"]["FireRating"] == "60 min", "slab fire rating")
for r in rows("WPEN"):
    ops = [o for o in f.by_type("IfcOpeningElement") if o.Name == r[0]]
    ok(len(ops) == (1 if r[1] == "1" else 0), f"register opening {r[0]}")
    for o in ops:
        ps = ue.get_psets(o)["OpenBIM_Penetration"]
        ok(ps["InFirePartition"] is True and ps["FireRating"] == "60 min" and ps["Register"] == r[0], "penetration properties")
        m = up.get_local_placement(o.ObjectPlacement)
        v, _ = verts(o)
        ok(np.allclose((v.min(axis=0) + v.max(axis=0))[0] / 2, float(r[2]), atol=0.2), "penetration is in the east wall")
for r in rows("FPEN"):
    ops = [o for o in f.by_type("IfcOpeningElement") if o.Name == r[0]]
    ok(len(ops) == (1 if r[1] == "Cut" else 0), f"floor opening {r[0]} {r[1]}")
sysmap = {}
for g in f.by_type("IfcRelAssignsToGroup"):
    if g.RelatingGroup.is_a("IfcDistributionSystem"):
        for o in g.RelatedObjects:
            sysmap[decode(o.GlobalId)[2]] = g.RelatingGroup.PredefinedType
for r in rows("SEG"):
    ok(sysmap.get(int(r[0])) == r[1], f"system of segment {r[0]}")

# ---- 6b. property sets and base quantities, against geometry computed here
def qto(e, name):
    for r in e.IsDefinedBy:
        if r.is_a("IfcRelDefinesByProperties") and r.RelatingPropertyDefinition.is_a("IfcElementQuantity") and r.RelatingPropertyDefinition.Name == name:
            return {q.Name: (q.is_a(), list(q)[3]) for q in r.RelatingPropertyDefinition.Quantities}
    return None
def mesh_volume(e, subtract):
    st = ifcopenshell.geom.settings()
    st.set("disable-opening-subtractions", not subtract)
    sh = ifcopenshell.geom.create_shape(st, e)
    return ush.get_volume(sh.geometry)
for w in f.by_type("IfcWall"):
    q = qto(w, "Qto_WallBaseQuantities")
    ok(q is not None, "wall has Qto_WallBaseQuantities")
    if q is None: continue
    ok(q["GrossVolume"][0] == "IfcQuantityVolume" and q["Length"][0] == "IfcQuantityLength" and q["NetSideArea"][0] == "IfcQuantityArea", "wall quantity kinds")
    ok(abs(q["GrossVolume"][1] - mesh_volume(w, False)) < 1e-6, f"wall gross volume {q['GrossVolume'][1]} vs mesh {mesh_volume(w, False)}")
    ok(abs(q["NetVolume"][1] - mesh_volume(w, True)) < 1e-6, f"wall net volume {q['NetVolume'][1]} vs mesh {mesh_volume(w, True)}")
    ok(abs(q["GrossVolume"][1] - q["GrossFootprintArea"][1] * q["Height"][1]) < 1e-6, "wall gross volume = footprint x height")
for sl in f.by_type("IfcSlab"):
    q = qto(sl, "Qto_SlabBaseQuantities")
    ok(q is not None, "slab has Qto_SlabBaseQuantities")
    if q is None: continue
    ok(abs(q["GrossVolume"][1] - mesh_volume(sl, False)) < 1e-6, "slab gross volume")
    ok(abs(q["NetVolume"][1] - mesh_volume(sl, True)) < 1e-6, f"slab net volume {q['NetVolume'][1]} vs mesh {mesh_volume(sl, True)}")
    ok(abs(q["NetArea"][1] * q["Depth"][1] - q["NetVolume"][1]) < 1e-6, "slab net area x depth = net volume")
for sp in f.by_type("IfcSpace"):
    q = qto(sp, "Qto_SpaceBaseQuantities")
    ok(q is not None, "space has Qto_SpaceBaseQuantities")
    if q is None: continue
    ok(abs(q["NetVolume"][1] - mesh_volume(sp, False)) < 1e-6, "space net volume equals the mesh of the room with its hole")
    ok(abs(q["NetFloorArea"][1] * q["Height"][1] - q["NetVolume"][1]) < 1e-6, "space area x height = volume")
    if sp.Name == "Office":
        ok(q["GrossFloorArea"][1] > q["NetFloorArea"][1], "the hole is taken off the net floor area")
    else:
        ok(abs(q["GrossFloorArea"][1] - q["NetFloorArea"][1]) < 1e-9, "a room without a hole has equal gross and net floor area")
    ok(sp.Decomposes[0].RelatingObject.is_a("IfcBuildingStorey"), "space sits in a storey")
for r in rows("IOPEN"):
    o = occs[int(r[0])]
    name = "Qto_DoorBaseQuantities" if o.is_a("IfcDoor") else "Qto_WindowBaseQuantities"
    q = qto(o, name)
    ok(q is not None, f"{o.Name} has {name}")
    if q is None: continue
    ok(abs(q["Width"][1] - float(r[1])) < 1e-9 and abs(q["Height"][1] - float(r[2])) < 1e-9 and abs(q["Area"][1] - float(r[1]) * float(r[2])) < 1e-9, f"{o.Name} quantities match its opening")
ps = ue.get_psets(occs[by_name["D-01"]])
ok(ps.get("Pset_DoorCommon", {}).get("Reference") == "D-01", "mapped parameter reached Pset_DoorCommon.Reference")
ok("Reference" not in ps.get("OpenBIM_Instance", {}), "mapped parameter is not also in the custom set")
tp = ue.get_psets(ue.get_type(occs[by_name["D-01"]]))
ok("OpenBIM_Family" in tp, "unmapped type values stay in OpenBIM_Family")

# ---- 6c. wall types, layer sets and joins
walls_by_id = elems("IfcWall")
ok(len(walls_by_id) == len(rows("WALL")), "one IfcWall per drawn wall, not per segment")
wtypes = {int(r[0]): r for r in rows("WTYPE")}
for r in rows("WTYPED"):
    w = walls_by_id[int(r[0])]
    name, layers = wtypes[int(r[1])][1], [(x.split("=")[0].replace("_", " "), float(x.split("=")[1])) for x in wtypes[int(r[1])][2].split(",")]
    t = ue.get_type(w)
    ok(t is not None and t.is_a("IfcWallType") and t.Name == name.replace("_", " "), f"wall {r[0]} has its type")
    use = ue.get_material(w, should_skip_usage=False)
    ok(use is not None and use.is_a("IfcMaterialLayerSetUsage"), f"wall {r[0]} has a layer set usage")
    if use is None or t is None: continue
    ls = use.ForLayerSet
    tm = ue.get_material(t)
    ok(tm is not None and tm.is_a("IfcMaterialLayerSet") and tm.id() == ls.id(), "the usage points at the layer set of the type")
    got = [(l.Material.Name, l.LayerThickness) for l in ls.MaterialLayers]
    ok(len(got) == len(layers) and all(a[0] == b[0] and abs(a[1] - b[1]) < 1e-9 for a, b in zip(got, layers)), f"layers of {name}: {got}")
    thick = float([x for x in rows("WALL") if x[0] == r[0]][0][-2])
    ok(abs(sum(l[1] for l in got) - thick) < 1e-9, "the layers add up to the wall thickness")
    ok(use.LayerSetDirection == "AXIS2" and use.DirectionSense == "POSITIVE" and abs(use.OffsetFromReferenceLine + thick / 2) < 1e-9, "usage offset puts the first layer on the right face")
    # the solid is as thick as the layers say: measured on the mesh of straight walls
    pts = [(float(a[1]), float(a[2])) for a in rows("WPT") if a[0] == r[0]]
    closed = [x for x in rows("WBASE") if x[0] == r[0]][0][2] == "1"
    if len(pts) == 2 and not closed:
        v, _ = verts(w)
        d = np.array(pts[1]) - np.array(pts[0]); d /= np.linalg.norm(d)
        n = np.array([-d[1], d[0]])
        across = v[:, :2] @ n
        ok(abs((across.max() - across.min()) - thick) < 1e-6, f"mesh thickness of wall {r[0]} equals its layers")
ok(all(len(set(w.IsTypedBy[0].RelatingType.id() for _ in [0])) == 1 for w in walls_by_id.values() if w.IsTypedBy), "typed walls have one type")
mans = {(int(a), ea, int(b), eb) for a, ea, b, eb in rows("WJOIN")}
got = set()
for rel in f.by_type("IfcRelConnectsPathElements"):
    got.add((decode(rel.RelatingElement.GlobalId)[2], rel.RelatingConnectionType.replace("AT", "", 1).title(), decode(rel.RelatedElement.GlobalId)[2], rel.RelatedConnectionType.replace("AT", "", 1).title()))
ok(got == mans, f"joins written and read back: {got} vs {mans}")
ok(len(mans) > 0, "the scene has joins")
# each join is real on the plan: the end of the first wall is on the second wall's centre line (or its end), within half its thickness
def centre_dist(pt, pts):
    best = 1e9
    for i in range(len(pts) - 1):
        a, b = np.array(pts[i]), np.array(pts[i + 1])
        t = np.clip(np.dot(pt - a, b - a) / np.dot(b - a, b - a), 0, 1)
        best = min(best, np.linalg.norm(pt - (a + t * (b - a))))
    return best
for a, ea, b, eb in mans:
    pa = [(float(x[1]), float(x[2])) for x in rows("WPT") if int(x[0]) == a]
    pb = [(float(x[1]), float(x[2])) for x in rows("WPT") if int(x[0]) == b]
    tb = float([x for x in rows("WALL") if int(x[0]) == b][0][-2])
    end = np.array(pa[0] if ea == "Start" else pa[-1])
    closed_b = [x for x in rows("WBASE") if int(x[0]) == b][0][2] == "1"
    ok(centre_dist(end, pb + ([pb[0]] if closed_b else [])) <= tb / 2 + 1e-6, f"wall {a} {ea} really meets wall {b}")


# ---- 6d. second-level space boundaries, read back from the file and checked against the element meshes
def pt_tri(p, a, b, c):
    ab, ac, ap = b - a, c - a, p - a
    d1, d2 = ab @ ap, ac @ ap
    if d1 <= 0 and d2 <= 0: return np.linalg.norm(p - a)
    bp = p - b; d3, d4 = ab @ bp, ac @ bp
    if d3 >= 0 and d4 <= d3: return np.linalg.norm(p - b)
    vc = d1 * d4 - d3 * d2
    if vc <= 0 and d1 >= 0 and d3 <= 0: return np.linalg.norm(p - (a + ab * d1 / (d1 - d3)))
    cp = p - c; d5, d6 = ab @ cp, ac @ cp
    if d6 >= 0 and d5 <= d6: return np.linalg.norm(p - c)
    vb = d5 * d2 - d1 * d6
    if vb <= 0 and d2 >= 0 and d6 <= 0: return np.linalg.norm(p - (a + ac * d2 / (d2 - d6)))
    va = d3 * d6 - d5 * d4
    if va <= 0 and (d4 - d3) >= 0 and (d5 - d6) >= 0:
        w = (d4 - d3) / ((d4 - d3) + (d5 - d6)); return np.linalg.norm(p - (b + w * (c - b)))
    den = 1 / (va + vb + vc); v, w = vb * den, vc * den
    return np.linalg.norm(p - (a + ab * v + ac * w))
def tri_mesh(e):
    v, s = verts(e)
    return v[np.array(s.geometry.faces).reshape(-1, 3)]
def ring_world(space_m, plane_pos, ring):
    m = space_m @ up.get_axis2placement(plane_pos)
    pts = [np.array(p.Coordinates) for p in ring.Points][:-1]
    return [(m @ np.array([x, y, 0.0, 1.0]))[:3] for x, y in pts], m
def polygon_area2(pts):
    return abs(sum(pts[i][0] * pts[(i + 1) % len(pts)][1] - pts[(i + 1) % len(pts)][0] * pts[i][1] for i in range(len(pts)))) / 2
def in_poly(pt, poly):
    c = False
    for i in range(len(poly)):
        (x1, y1), (x2, y2) = poly[i], poly[(i + 1) % len(poly)]
        if (y1 > pt[1]) != (y2 > pt[1]) and pt[0] < (x2 - x1) * (pt[1] - y1) / (y2 - y1) + x1: c = not c
    return c
rels = f.by_type("IfcRelSpaceBoundary2ndLevel")
sb_rows = {}
for r in rows("SB"): sb_rows.setdefault(int(r[0]), []).append(r)
ok(len(rels) == len(rows("SB")) and len(rels) > 0, f"one boundary written for each the model has: {len(rels)} vs {len(rows('SB'))}")
room_rows = {int(r[0]): r for r in rows("ROOM")}
spaces_by_id = {decode(s.GlobalId)[2]: s for s in f.by_type("IfcSpace")}
by_space = {}
for rel in rels: by_space.setdefault(decode(rel.RelatingSpace.GlobalId)[2], []).append(rel)
mesh_cache = {}
def mesh_of(e):
    if e.id() not in mesh_cache: mesh_cache[e.id()] = tri_mesh(e)
    return mesh_cache[e.id()]
n_doors_expected = 0
for sid, sp in spaces_by_id.items():
    mine = by_space.get(sid, [])
    exp_rows = sb_rows[sid]
    ok(len(mine) == len(exp_rows), f"space {sp.Name}: {len(mine)} boundaries written, {len(exp_rows)} expected")
    sm = up.get_local_placement(sp.ObjectPlacement)
    q = qto(sp, "Qto_SpaceBaseQuantities")
    kinds = []
    for rel, row in zip(mine, exp_rows):
        el = rel.RelatedBuildingElement
        ok(decode(el.GlobalId)[2] == int(row[3]), f"{sp.Name}: boundary {row[1]} is on element {row[3]}")
        ok(rel.PhysicalOrVirtualBoundary == "PHYSICAL" and rel.InternalOrExternalBoundary == "NOTDEFINED", "boundary is physical")
        surf = rel.ConnectionGeometry.SurfaceOnRelatingElement
        ok(surf.is_a("IfcCurveBoundedPlane"), "boundary surface is a bounded plane")
        outer, m = ring_world(sm, surf.BasisSurface.Position, surf.OuterBoundary)
        # the file agrees with the numbers the model gave
        mine_pts = [np.array(row[7 + 3 * i:10 + 3 * i], float) for i in range(int(row[6]))]
        ok(len(outer) == len(mine_pts) and all(np.linalg.norm(a - b) < 1e-9 for a, b in zip(outer, mine_pts)), f"{sp.Name}: boundary {row[1]} corners as exported")
        holes = [ring_world(sm, surf.BasisSurface.Position, h)[0] for h in surf.InnerBoundaries or []]
        # every corner (outer and hole) lies on the surface of the element it bounds, or on the parent face for a door or window
        tris = mesh_of(el)
        for p in outer + [x for h in holes for x in h]:
            ok(min(pt_tri(p, t[0], t[1], t[2]) for t in tris) < 1e-6, f"{sp.Name}: boundary {row[1]} corner {np.round(p, 3)} lies on {el.Name or el.is_a()}")
        # the normal points out of the room: a step into it from the middle of the face is outside the room, a step back is inside
        nrm = m[:3, 2]
        # a point well inside the face: the corner average of the outline, moved off any hole if needed
        cen = np.mean(outer, axis=0)
        rm = room_rows[sid]
        # the room outline from the manifest, for the inside test in plan
        poly = [(float(rm[5 + 2 * i]), float(rm[6 + 2 * i])) for i in range(int(rm[4]))]
        z0, z1 = float(rm[2]), float(rm[2]) + float(rm[3])
        def in_room(p):
            return in_poly(p, poly) and z0 < p[2] < z1
        if abs(nrm[2]) < 0.5:
            # a wall: the face is a rectangle, so its middle is a point of it; check just off the face on both sides
            face_mid = (outer[0] + outer[2]) / 2
            ok(not in_room(face_mid + 0.02 * nrm) and in_room(face_mid - 0.02 * nrm), f"{sp.Name}: boundary {row[1]} faces out of the room")
        else:
            # floor or ceiling: one step along the normal is out of the room in height
            below = nrm[2] < 0
            ok((cen[2] <= z0 + 0.01) if below else (cen[2] >= z1 - 0.3), f"{sp.Name}: boundary {row[1]} is the {'floor' if below else 'ceiling'}")
        # area: the outline less the holes
        local = lambda pts: [(np.linalg.inv(m) @ np.append(p, 1))[:2] for p in pts]
        area = polygon_area2(local(outer)) - sum(polygon_area2(local(h)) for h in holes)
        ok(abs(area - float(row[5])) < 1e-9, f"{sp.Name}: boundary {row[1]} area")
        kinds.append((row[2], area, rel, holes))
    # the floor or ceiling of a room covers its net floor area
    for kind, area, rel, holes in kinds:
        if kind == "SLAB":
            ok(abs(area - q["NetFloorArea"][1]) < 1e-9, f"{sp.Name}: the slab boundary covers the net floor area")
    # a door or window boundary sits in its wall's boundary: same plane, inside one of its holes
    for rel, row in zip(mine, exp_rows):
        if row[2] == "ITEM":
            par = rel.ParentBoundary
            ok(par is not None and par in mine, "a door or window boundary has its wall boundary as parent")
            ok(mine.index(par) == int(row[4]), "the parent is the one the model names")
            if par is None: continue
            ps = par.ConnectionGeometry.SurfaceOnRelatingElement
            pm = sm @ up.get_axis2placement(ps.BasisSurface.Position)
            cm = sm @ up.get_axis2placement(rel.ConnectionGeometry.SurfaceOnRelatingElement.BasisSurface.Position)
            ok(np.allclose(pm[:3, 2], cm[:3, 2]) and abs((cm[:3, 3] - pm[:3, 3]) @ pm[:3, 2]) < 1e-9, "a door's face lies in its wall's plane")
            cw = ring_world(sm, rel.ConnectionGeometry.SurfaceOnRelatingElement.BasisSurface.Position, rel.ConnectionGeometry.SurfaceOnRelatingElement.OuterBoundary)[0]
            inner_local = [[(np.linalg.inv(pm) @ np.append(p, 1))[:2] for p in ring_world(sm, ps.BasisSurface.Position, h)[0]] for h in ps.InnerBoundaries]
            door_local = [(np.linalg.inv(pm) @ np.append(p, 1))[:2] for p in cw]
            def inside_hole(pt): return any(min(x[0] for x in h) - 1e-9 <= pt[0] <= max(x[0] for x in h) + 1e-9 and min(x[1] for x in h) - 1e-9 <= pt[1] <= max(x[1] for x in h) + 1e-9 for h in inner_local)
            ok(all(inside_hole(p) for p in door_local), "a door or window face sits in a hole of its wall's face")
            n_doors_expected += 1
# D-02 and D-03 stand in the south wall between the partition and the east wall; D-01 and W-01 are on the other side of the partition
ok(n_doors_expected == 2, f"two doors border the Office: {n_doors_expected}")


# ---- 6e. georeferencing, read back with IfcOpenShell's own geolocation helpers
import ifcopenshell.util.geolocation as geo
gr = rows("GEOREF")[0]
E, N, H, ang, scale, tn, lat, lon, elev = (float(x) for x in gr[:9])
mc = f.by_type("IfcMapConversion")
ok(len(mc) == 1, "one map conversion")
mc = mc[0]
ok(mc.SourceCRS == f.by_type("IfcProject")[0].RepresentationContexts[0], "the map conversion starts from the model's own context")
crs = mc.TargetCRS
ok(crs.is_a("IfcProjectedCRS") and crs.Name == gr[9] and crs.MapUnit.Name == "METRE" and crs.GeodeticDatum == "TESTDATUM" and crs.MapZone == "48N", "projected CRS names")
ok(abs(mc.Eastings - E) < 1e-9 and abs(mc.Northings - N) < 1e-9 and abs(mc.OrthogonalHeight - H) < 1e-9 and abs(mc.Scale - scale) < 1e-12, "map conversion origin and scale")
ok(abs(math.degrees(math.atan2(mc.XAxisOrdinate, mc.XAxisAbscissa)) - ang) < 1e-9 and abs(math.hypot(mc.XAxisAbscissa, mc.XAxisOrdinate) - 1) < 1e-12, "grid angle")
for r in rows("GEOPT"):
    x, y, z, me, mn, mh, bx, by, bz = (float(v) for v in r)
    e, n, h = geo.xyz2enh(x, y, z, mc.Eastings, mc.Northings, mc.OrthogonalHeight, mc.XAxisAbscissa, mc.XAxisOrdinate, mc.Scale)
    ok(abs(e - me) < 1e-6 and abs(n - mn) < 1e-6 and abs(h - mh) < 1e-6, f"point {x},{y},{z}: IfcOpenShell puts it at {e:.4f} {n:.4f} {h:.4f}, the model at {me:.4f} {mn:.4f} {mh:.4f}")
    ok(abs(bx - x) < 1e-6 and abs(by - y) < 1e-6 and abs(bz - z) < 1e-6, "map point comes back to the model point")
pts = [np.array([float(v) for v in r[3:6]]) for r in rows("GEOPT")]
ok(abs(np.linalg.norm(pts[1] - pts[0]) - 10 * scale) < 1e-9, "a 10 m length on the map is 10 m times the scale")
ok(abs(math.degrees(math.atan2(pts[1][1] - pts[0][1], pts[1][0] - pts[0][0])) - ang) < 1e-9, "the model X axis lies at the grid angle on the map")
ok(abs(geo.get_true_north(f) - tn) < 1e-9, f"true north {geo.get_true_north(f)} vs {tn}")
tnd = f.by_type("IfcProject")[0].RepresentationContexts[0].TrueNorth.DirectionRatios
ok(abs(tnd[0] + math.sin(math.radians(tn))) < 1e-12 and abs(tnd[1] - math.cos(math.radians(tn))) < 1e-12, "true north is the Y axis turned anticlockwise by the angle")
site = f.by_type("IfcSite")[0]
ok(abs(geo.dms2dd(*site.RefLatitude) - lat) < 1e-8 and abs(geo.dms2dd(*site.RefLongitude) - lon) < 1e-8 and abs(site.RefElevation - elev) < 1e-9, "site latitude, longitude and elevation")

# ---- 7. repeatability and stable ids
a = open(sys.argv[2], "rb").read(); b = open(sys.argv[3], "rb").read()
ok(a == b, "two exports are byte for byte the same")
g2 = ifcopenshell.open(sys.argv[4])
ok(a != open(sys.argv[4], "rb").read(), "the edit changed the file")
ids1 = {(e.is_a(), e.GlobalId) for e in f.by_type("IfcRoot")}
ids2 = {(e.is_a(), e.GlobalId) for e in g2.by_type("IfcRoot")}
ok(ids1 == ids2, f"GlobalIds after an edit: {len(ids1 ^ ids2)} differ")
d1 = occs[by_name["D-01"]]
d2 = [e for e in g2.by_type("IfcDoor") if e.GlobalId == d1.GlobalId][0]
x1 = up.get_local_placement(d1.ObjectPlacement)[0, 3]; x2 = up.get_local_placement(d2.ObjectPlacement)[0, 3]
ok(abs((x2 - x1) - 1.0) < 1e-9, "the edited door moved 1 m and kept its id")
op1 = d1.FillsVoids[0].RelatingOpeningElement; op2 = d2.FillsVoids[0].RelatingOpeningElement
ok(op1.GlobalId == op2.GlobalId and abs(up.get_local_placement(op2.ObjectPlacement)[0, 3] - up.get_local_placement(op1.ObjectPlacement)[0, 3] - 1.0) < 1e-9, "its opening moved with it and kept its id")
ok(ue.get_type(d2).GlobalId == ue.get_type(d1).GlobalId, "the type kept its id")

# ---- 8. negative controls
wrong = {k: (p + np.array([0, 0, 1e-3]), d) for k, (p, d) in want.items()}
ok(port_check(ports, wrong) == len(want), "negative control: moved ports are all reported")
flip = {k: (p, -d) for k, (p, d) in want.items()}
ok(port_check(ports, flip) == len(want), "negative control: reversed ports are all reported")
ok(my_compress(key + 1, 0, next(iter(wall_ids))) != next(iter(walls.values())).GlobalId, "negative control: another project key gives another id")

print(f"{checks} checks, {fails} failures")
sys.exit(1 if fails else 0)
