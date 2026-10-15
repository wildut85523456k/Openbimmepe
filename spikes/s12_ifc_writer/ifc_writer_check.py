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
