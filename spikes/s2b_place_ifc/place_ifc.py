"""Spike S2b cross-check: write the Rust scene (walls + placed doors and windows) as IFC4, reopen it,
and compare against the Rust numbers using IfcOpenShell / OpenCASCADE.

Usage: python3 place_ifc.py scene.txt
The scene file refuses to be used when incomplete (no TOTAL line)."""
import sys, os, tempfile, collections
import numpy as np
import ifcopenshell, ifcopenshell.api, ifcopenshell.geom, ifcopenshell.util.element as E

api = ifcopenshell.api.run
lines = [l.split() for l in open(sys.argv[1]).read().strip().split("\n")]
by = collections.defaultdict(list)
for l in lines: by[l[0]].append(l)
if not by["TOTAL"]: sys.exit("scene file is incomplete (no TOTAL line)")
fl = lambda xs: [float(x) for x in xs]

f = api("project.create_file", version="IFC4")
proj = api("root.create_entity", f, ifc_class="IfcProject", name="S2b spike")
api("unit.assign_unit", f, length={"is_metric": True, "raw": "METERS"})
ctx = api("context.add_context", f, context_type="Model")
body = api("context.add_context", f, context_type="Model", context_identifier="Body", target_view="MODEL_VIEW", parent=ctx)
site = api("root.create_entity", f, ifc_class="IfcSite", name="Site")
bld = api("root.create_entity", f, ifc_class="IfcBuilding", name="Building")
api("aggregate.assign_object", f, products=[site], relating_object=proj)
api("aggregate.assign_object", f, products=[bld], relating_object=site)
storeys = {}
def storey(z):
    if z not in storeys:
        s = api("root.create_entity", f, ifc_class="IfcBuildingStorey", name=f"Level {len(storeys)}")
        s.Elevation = z
        api("aggregate.assign_object", f, products=[s], relating_object=bld)
        storeys[z] = s
    return storeys[z]

def place(el, m): api("geometry.edit_object_placement", f, product=el, matrix=np.array(m))

# ---- walls: one per segment, extruded mitred plan quad
walls = {}
base_of_chain = {}
for l in by["SEG"]:
    ci, si = int(l[1]), int(l[2]); v = fl(l[3:])
    l0, l1, r1, r0 = (v[0], v[1]), (v[2], v[3]), (v[4], v[5]), (v[6], v[7])
    height, base = v[8], v[9]
    base_of_chain[ci] = base
    pts = [r0, r1, l1, l0]                                  # counter-clockwise
    poly = f.createIfcPolyline([f.createIfcCartesianPoint(p) for p in pts + [pts[0]]])
    prof = f.createIfcArbitraryClosedProfileDef("AREA", None, poly)
    w = api("root.create_entity", f, ifc_class="IfcWall", name=f"Wall c{ci}s{si}")
    rep = api("geometry.add_profile_representation", f, context=body, profile=prof, depth=height, cardinal_point=None)
    api("geometry.assign_representation", f, product=w, representation=rep)
    place(w, np.eye(4) + np.diag([0, 0, 0, 0]) * 0 + np.array([[0,0,0,0],[0,0,0,0],[0,0,0,base],[0,0,0,0]]))
    api("spatial.assign_container", f, products=[w], relating_structure=storey(base))
    walls[(ci, si)] = (w, v[10])

# ---- placed families
def corners(org, ax, ay, size):
    o = np.array(org); X = np.array([ax[0], ax[1], 0.0]); Y = np.array([ay[0], ay[1], 0.0]); Z = np.array([0, 0, 1.0])
    return [o + X*size[0]*(c & 1) + Y*size[1]*((c >> 1) & 1) + Z*size[2]*((c >> 2) & 1) for c in range(8)]
FACES = [(0,1,3,2),(4,6,7,5),(0,4,5,1),(2,3,7,6),(0,2,6,4),(1,5,7,3)]
def outward(vs):
    c = sum(vs) / 8.0; out = []
    for q in FACES:
        n = np.cross(vs[q[1]] - vs[q[0]], vs[q[2]] - vs[q[0]]); mid = sum(vs[i] for i in q) / 4.0
        out.append(list(q) if np.dot(n, mid - c) > 0 else list(reversed(q)))
    return out

placed = {}
for l in by["PLACED"]:
    k = int(l[1]); placed[k] = dict(chain=int(l[2]), seg=int(l[3]), fam=l[4].replace("_", " "), typ=l[5].replace("_", " "), centre=float(l[6]), vals={}, boxes=[], opening=None)
for l in by["VAL"]: placed[int(l[1])]["vals"][l[2]] = float(l[3])
for l in by.get("DERIVED", []): placed[int(l[1])].setdefault("derived", {})[l[2]] = float(l[3])
for l in by["TEXT"]: placed[int(l[1])].setdefault("texts", {})[l[2]] = "" if l[3] == "-" else l[3].replace("_", " ")
for l in by["MAT"]: placed[int(l[1])].setdefault("mats", {})[int(l[2])] = None if l[3] == "-" else l[3].replace("_", " ")
for l in by["OPENING"]: placed[int(l[1])]["opening"] = fl(l[2:])
openpoly = {int(l[1]): l for l in by.get("OPENPOLY", [])}
for l in by["BOX"]: placed[int(l[1])]["boxes"].append((l[2], fl(l[3:])))
fam_expected = {int(l[1]): float(l[2]) for l in by["FAMVOL"]}

elements = {}
mats = {}
for k, p in sorted(placed.items()):
    wall, _ = walls[(p["chain"], p["seg"])]
    base = base_of_chain[p["chain"]]
    cls, pre = ("IfcDoor", "DOOR") if p["fam"].startswith("Door") else ("IfcWindow", "WINDOW")
    el = api("root.create_entity", f, ifc_class=cls, name=f'{p["fam"]} - {p["typ"]}', predefined_type=pre)
    verts, faces = [], []
    for name, b in p["boxes"]:
        vs = corners(b[0:3], b[3:5], b[5:7], b[7:10])
        faces.extend([[ [i + len(verts) for i in q] ] for q in outward(vs)] if False else [[i + len(verts) for i in q] for q in outward(vs)])
        verts.extend([list(v) for v in vs])
    rep = api("geometry.add_mesh_representation", f, context=body, vertices=[verts], faces=[faces])
    api("geometry.assign_representation", f, product=el, representation=rep)
    place(el, np.eye(4))
    api("spatial.assign_container", f, products=[el], relating_structure=storey(base))
    el.OverallWidth = p["vals"]["width"]; el.OverallHeight = p["vals"]["height"]
    if cls == "IfcWindow" and "u_window" in p.get("derived", {}):
        pw = api("pset.add_pset", f, product=el, name="Pset_WindowCommon")
        api("pset.edit_pset", f, pset=pw, properties={"ThermalTransmittance": p["derived"]["u_window"], "IsExternal": True})
    ps = api("pset.add_pset", f, product=el, name="OpenBIM_Family")
    api("pset.edit_pset", f, pset=ps, properties={"Family": p["fam"], "Type": p["typ"], "PlacementIndex": k, **{n: float(v) for n, v in p["vals"].items()}, **{"text_" + n: v for n, v in p["texts"].items()}})
    # one material constituent per distinct material of the parts (the mesh is one representation)
    cset = api("material.add_material_set", f, name=f'{p["fam"]} parts', set_type="IfcMaterialConstituentSet")
    for mname in sorted({m for m in p["mats"].values() if m}):
        mat = mats.get(mname) or mats.setdefault(mname, api("material.add_material", f, name=mname))
        c = api("material.add_constituent", f, constituent_set=cset, material=mat, name=mname)
    api("material.assign_material", f, products=[el], type="IfcMaterialConstituentSet", material=cset)
    if p["opening"]:
        o = p["opening"]
        op = api("root.create_entity", f, ifc_class="IfcOpeningElement", name=f"Opening {k}")
        if k in openpoly:
            # arched head: profile drawn in the wall's (s, z) plane and extruded through the wall thickness
            q = openpoly[k]; ob = fl(q[2:5]); uu = fl(q[5:7]); nn = fl(q[7:9]); depth = float(q[9]); npt = int(q[10]); pts = fl(q[11:11 + 2 * npt])
            ring = [f.createIfcCartesianPoint((pts[2 * i], pts[2 * i + 1])) for i in range(npt)]
            prof = f.createIfcArbitraryClosedProfileDef("AREA", None, f.createIfcPolyline(ring + [ring[0]]))
            orep = api("geometry.add_profile_representation", f, context=body, profile=prof, depth=depth, cardinal_point=None)
            api("geometry.assign_representation", f, product=op, representation=orep)
            m = np.eye(4); m[:3, 3] = ob
            m[:3, 0] = [uu[0], uu[1], 0.0]; m[:3, 1] = [0.0, 0.0, 1.0]; m[:3, 2] = [-nn[0], -nn[1], 0.0]
            place(op, m)
        else:
            prof = f.createIfcRectangleProfileDef("AREA", None, f.createIfcAxis2Placement2D(f.createIfcCartesianPoint((o[7] / 2, o[8] / 2))), o[7], o[8])
            orep = api("geometry.add_profile_representation", f, context=body, profile=prof, depth=o[9], cardinal_point=None)
            api("geometry.assign_representation", f, product=op, representation=orep)
            m = np.eye(4); m[:3, 3] = o[0:3]; m[0, 0], m[1, 0] = o[3], o[4]; m[0, 1], m[1, 1] = o[5], o[6]
            place(op, m)
        api("feature.add_feature", f, feature=op, element=wall)
        api("feature.add_filling", f, opening=op, element=el)
        api("spatial.assign_container", f, products=[op], relating_structure=storey(base)) if False else None
    elements[k] = el

path = os.path.join(tempfile.mkdtemp(), "s2b.ifc"); f.write(path)
g = ifcopenshell.open(path)

# ---- read back and compare
S = ifcopenshell.geom.settings(); S.set("use-world-coords", True)
S0 = ifcopenshell.geom.settings(); S0.set("use-world-coords", True); S0.set("disable-opening-subtractions", True)
def shape(model_el, s):
    sh = ifcopenshell.geom.create_shape(s, model_el)
    v = np.array(sh.geometry.verts).reshape(-1, 3); fc = np.array(sh.geometry.faces).reshape(-1, 3)
    return v, fc
def volume(v, fc):
    a, b, c = v[fc[:, 0]], v[fc[:, 1]], v[fc[:, 2]]
    return abs(np.einsum("ij,ij->i", a, np.cross(b, c)).sum() / 6)

checks = []
def chk(name, a, b, tol=None):
    ok = (a == b) if tol is None else (abs(a - b) <= tol * (1 + abs(a)))
    checks.append((ok, name, a, b))

nplaced_open = sum(1 for p in placed.values() if p["opening"])
chk("walls", len(g.by_type("IfcWall")), len(walls))
chk("doors", len(g.by_type("IfcDoor")), sum(1 for p in placed.values() if p["fam"].startswith("Door")))
chk("windows", len(g.by_type("IfcWindow")), sum(1 for p in placed.values() if p["fam"].startswith("Window")))
chk("openings", len(g.by_type("IfcOpeningElement")), nplaced_open)
chk("storeys", len(g.by_type("IfcBuildingStorey")), len(set(base_of_chain.values())))

# every door/window fills an opening that voids the intended wall
ok_rel = True
for e in g.by_type("IfcDoor") + g.by_type("IfcWindow"):
    k = E.get_psets(e)["OpenBIM_Family"]["PlacementIndex"]
    p = placed[int(k)]
    hosts = [r.RelatingBuildingElement.Name for fl_ in e.FillsVoids for r in fl_.RelatingOpeningElement.VoidsElements]
    ok_rel &= hosts == [f'Wall c{p["chain"]}s{p["seg"]}']
chk("each door/window fills an opening in its own wall", ok_rel, True)

# property sets survive
ok_ps = True
for e in g.by_type("IfcDoor") + g.by_type("IfcWindow"):
    ps = E.get_psets(e)["OpenBIM_Family"]; p = placed[int(ps["PlacementIndex"])]
    ok_ps &= all(abs(ps[n] - v) < 1e-12 for n, v in p["vals"].items()) and ps["Family"] == p["fam"] and ps["Type"] == p["typ"]
    ok_ps &= abs(e.OverallWidth - p["vals"]["width"]) < 1e-12 and abs(e.OverallHeight - p["vals"]["height"]) < 1e-12
chk("family values, type and size attributes read back", ok_ps, True)

# texts and materials read back
ok_tx = ok_mt = True
for e in g.by_type("IfcDoor") + g.by_type("IfcWindow"):
    ps = E.get_psets(e)["OpenBIM_Family"]; p = placed[int(ps["PlacementIndex"])]
    ok_tx &= all(ps["text_" + n] == v for n, v in p["texts"].items())
    got = sorted(c.Name for c in e.HasAssociations[0].RelatingMaterial.MaterialConstituents)
    ok_mt &= got == sorted({m for m in p["mats"].values() if m})
    ok_mt &= all(c.Material.Name == c.Name for c in e.HasAssociations[0].RelatingMaterial.MaterialConstituents)
chk("text values (mark, swing, ...) read back from the property set", ok_tx, True)
chk("materials read back as constituents", ok_mt, True)
chk("door D-01 is left-hinged", any(E.get_psets(e)["OpenBIM_Family"].get("text_swing") == "left" for e in g.by_type("IfcDoor")), True)

# expectations written by hand, not taken from the scene file (door 0: left-hinged, centre 2.0 on the wall from (0,0) along +x)
d0 = [e for e in g.by_type("IfcDoor") if E.get_psets(e)["OpenBIM_Family"]["PlacementIndex"] == 0][0]
ps0 = E.get_psets(d0)["OpenBIM_Family"]
chk("hand: door 0 mark and swing", (ps0["text_mark"], ps0["text_swing"]), ("D-01", "left"))
chk("hand: door 0 materials", sorted(c.Name for c in d0.HasAssociations[0].RelatingMaterial.MaterialConstituents), ["Frame material", "Handle finish", "Leaf material"])
v0, _ = shape(d0, S)
def has(x, y, z): return bool(np.any(np.all(np.abs(v0 - np.array([x, y, z])) < 1e-9, axis=1)))
# plate 0.04 wide at x = 2.0 + 0.31..0.35, leaf face at y = -0.02, plate 0.008 thick, z 0.92..1.08; lever from x 2.23
chk("hand: handle plate corners", [has(x, y, z) for x in (2.31, 2.35) for y in (-0.028, -0.02) for z in (0.92, 1.08)], [True] * 8)
chk("hand: lever tip at x 2.23, 0.078 in front of the leaf", has(2.23, -0.078, 0.99) and has(2.23, -0.078, 1.01), True)
chk("hand: no handle part at the hinge-free right-hinged position", has(1.65, -0.028, 0.92), False)

# door 4: Type B (width 1.2, frame 0.05, height 2.4), right-hinged, open 60 degrees, on segment 3 of the room (from (0,5) to (0,0)), centre 2.5.
# Door frame origin (0, 2.5); wall direction u = (0,-1), left normal (1,0): world = (0 + y, 2.5 - x).
# The hinge is at x = 0.55; the free edge centre (-0.55, 0) turns by -60 degrees about it to (0.0, 0.9526...) in the door frame;
# the leaf is 0.04 thick along the turned y axis (0.866, 0.5): corners (+-0.01732, 0.95263 +- 0.01) -> world (0.95263 +- 0.01, 2.5 -+ 0.01732)
d4 = [e for e in g.by_type("IfcDoor") if E.get_psets(e)["OpenBIM_Family"]["PlacementIndex"] == 4][0]
v4, _ = shape(d4, S)
def has4(x, y, z): return bool(np.any(np.all(np.abs(v4 - np.array([x, y, z])) < 1e-4, axis=1)))
t = 1.1 * np.sin(np.radians(60))           # = 0.95263
chk("hand: open door free edge corners (bottom and top of the leaf)",
    [has4(t + sx * 0.01, 2.5 - sx * 0.02 * np.cos(np.radians(30)), z) for sx in (1, -1) for z in (0.0, 2.35)], [True] * 4)
chk("hand: open door hinge edge corners stay at the hinge",
    [has4(0.01, 1.95 - 0.01732, z) and has4(-0.01, 1.95 + 0.01732, z) for z in (0.0, 2.35)], [True, True])
chk("hand: open door has 4 frame parts + leaf only (no handle: has_handle = 0)", len(placed[4]["boxes"]), 4)

# wall volumes: IfcOpenShell/OpenCASCADE boolean against Rust exact, per segment and in total
tot = 0.0; worst = 0.0
for w in g.by_type("IfcWall"):
    nm = w.Name; ci, si = int(nm.split("c")[1].split("s")[0]), int(nm.split("s")[-1])
    net = volume(*shape(w, S)); raw = volume(*shape(w, S0))
    tot += net
    worst = max(worst, abs(net - walls[(ci, si)][1]))
    cuts = sum(p["opening"][10] for p in placed.values() if p["opening"] and p["chain"] == ci and p["seg"] == si)
    chk(f"{nm}: net volume vs Rust exact", net, walls[(ci, si)][1], 1e-6)
    chk(f"{nm}: uncut minus cut volume of its openings", raw - cuts, net, 1e-6)
chk("total net wall volume vs Rust", tot, float(by["TOTAL"][0][1]), 1e-6)

# door/window geometry: volume and position inside its opening
for e in g.by_type("IfcDoor") + g.by_type("IfcWindow"):
    k = int(E.get_psets(e)["OpenBIM_Family"]["PlacementIndex"]); p = placed[k]
    v, fc = shape(e, S)
    chk(f"placement {k} ({p['fam']}): part volume vs Rust", volume(v, fc), fam_expected[k], 1e-9)
    nparts = len(p["boxes"]); chk(f"placement {k}: every part has a material entry", sorted(p["mats"]) , list(range(nparts)))
    chk(f"placement {k}: part count equals mesh components", len(shape(e, S)[1]) // 12, nparts)
    o = p["opening"]
    turned = p["vals"].get("open_angle", 0.0) != 0.0
    if o and turned:
        # a door standing open swings out of the wall: only the part volume (above) and the hand-calculated leaf position are checked
        pass
    elif o:
        ax = np.array([o[3], o[4]]); ay = np.array([o[5], o[6]]); org = np.array(o[0:2])
        framebox = [i for i, (nm, _) in enumerate(p["boxes"]) if "/" not in nm]
        vf = np.vstack([np.array([corners(b[0:3], b[3:5], b[5:7], b[7:10])]).reshape(-1, 3) for nm, b in p["boxes"] if "/" not in nm])
        rel = (vf[:, :2] - org) @ np.array([ax, ay]).T         # coordinates in the opening frame (frame parts only)
        lo, hi = rel.min(0), rel.max(0)
        vh = [corners(b[0:3], b[3:5], b[5:7], b[7:10]) for nm, b in p["boxes"] if "/" in nm]
        if vh:
            vh = np.vstack(vh); rh = (vh[:, :2] - org) @ np.array([ax, ay]).T
            chk(f"placement {k}: handle stays inside the opening width", bool(rh[:, 0].min() >= -1e-9 and rh[:, 0].max() <= o[7] + 1e-9), True)
            chk(f"placement {k}: handle is between the sill and the head", bool(vh[:, 2].min() >= o[2] and vh[:, 2].max() <= o[2] + o[9]), True)
            # swing: right-hinged door has its handle on the low-x side of the opening, left-hinged on the high side
            mid = o[7] / 2; hx = (rh[:, 0].min() + rh[:, 0].max()) / 2
            chk(f"placement {k}: handle sits on the side opposite the hinges ({p['texts']['swing']})", bool(hx < mid) if p["texts"]["swing"] == "right" else bool(hx > mid), True)
        chk(f"placement {k}: parts span the opening width", (round(lo[0], 6), round(hi[0], 6)), (0.0, round(o[7], 6)))
        chk(f"placement {k}: parts stay inside the wall thickness", bool(lo[1] >= -1e-9 and hi[1] <= o[8] + 1e-9), True)
        chk(f"placement {k}: parts rise from the sill to the head or below", bool(v[:, 2].min() >= org_z - 1e-9) if (org_z := o[2]) is not None else False, True)
        chk(f"placement {k}: parts reach the head of the opening (the springing line of an arch)", round(v[:, 2].max(), 6), round(o[2] + o[9] - p["vals"].get("rise", 0.0), 6))

# arched cuts: hand formula (rectangle + fan of 32 thin triangles - centre triangle), from the family values only
import math
for k, q in sorted(openpoly.items()):
    p = placed[k]; w_, h_, r_ = p["vals"]["width"], p["vals"]["height"], p["vals"]["rise"]
    half = w_ / 2; R = (half * half + r_ * r_) / (2 * r_); al = math.asin(half / R); n_ = 32
    area = w_ * (h_ - r_) + 0.5 * R * R * (n_ * math.sin(2 * al / n_) - math.sin(2 * al))
    thick = p["opening"][8] - 0.1
    chk(f"placement {k}: arched cut volume equals the hand formula", p["opening"][10], area * thick, 1e-9)
    chk(f"placement {k}: arch polygon has 35 points", int(q[10]), 4 + 31)

# window U-value: recomputed here from the values stored in the file, with the formula written out again in Python
for e in g.by_type("IfcWindow"):
    k = int(E.get_psets(e)["OpenBIM_Family"]["PlacementIndex"]); p = placed[k]; v = {n: E.get_psets(e)["OpenBIM_Family"][n] for n in ("width", "height", "frame", "mullion_w", "ug", "uf", "psi")}
    n_bars = E.get_psets(e)["OpenBIM_Family"]["mullions"]
    pane_w = (v["width"] - 2 * v["frame"] - n_bars * v["mullion_w"]) / (n_bars + 1); pane_h = v["height"] - 2 * v["frame"]
    area = v["width"] * v["height"]; ag = (n_bars + 1) * pane_w * pane_h; lg = (n_bars + 1) * 2 * (pane_w + pane_h)
    uw = (ag * v["ug"] + (area - ag) * v["uf"] + lg * v["psi"]) / area
    chk(f"placement {k}: window U in the file vs recomputed from its values", E.get_psets(e)["Pset_WindowCommon"]["ThermalTransmittance"], uw, 1e-9)
# placement 1 by hand with the default values: 1.2 x 1.2, frame 0.05, bars 0.03 wide, Ug 1.1, Uf 2.0, psi 0.06, and 2 bars (3 panes)
chk("placement 1 (2 bars, 1.2 x 1.2): window U by hand", E.get_psets([e for e in g.by_type("IfcWindow") if int(E.get_psets(e)["OpenBIM_Family"]["PlacementIndex"]) == 1][0])["Pset_WindowCommon"]["ThermalTransmittance"],
    ((3 * ((1.2 - 0.1 - 2 * 0.03) / 3) * 1.1) * 1.1 + (1.44 - 3 * ((1.2 - 0.1 - 2 * 0.03) / 3) * 1.1) * 2.0 + 3 * 2 * (((1.2 - 0.1 - 2 * 0.03) / 3) + 1.1) * 0.06) / 1.44, 1e-9)

# the cuts S1 actually made: the net wall must have vertices at the corners of every cut, on both faces
for w in g.by_type("IfcWall"):
    nm = w.Name; ci, si = int(nm.split("c")[1].split("s")[0]), int(nm.split("s")[-1])
    ln = [l for l in by["LINE"] if int(l[1]) == ci and int(l[2]) == si][0]
    x0, y0, x1, y1, t = fl(ln[3:8]); d = np.array([x1 - x0, y1 - y0]); L = np.hypot(*d); u = d / L; nlv = np.array([-u[1], u[0]])
    v, _ = shape(w, S)
    for c_ in [c for c in by["CUT"] if int(c[1]) == ci and int(c[2]) == si]:
        off, wd, sill, ht = fl(c_[3:7]); found = 0; want = 0
        for s_ in (off, off + wd):
            for z in (base_of_chain[ci] + sill, base_of_chain[ci] + sill + ht):
                for side in (-t / 2, t / 2):
                    p_ = np.array([x0, y0]) + u * s_ + nlv * side; want += 1
                    found += bool(np.any(np.all(np.abs(v - np.array([p_[0], p_[1], z])) < 1e-6, axis=1)))
        chk(f"{nm}: wall has vertices at all 8 corners of the cut at s={off:.3f}", (found, want), (8, 8))
    for c_ in [c for c in by.get("CUTP", []) if int(c[1]) == ci and int(c[2]) == si]:
        n_ = int(c_[3]); pts = fl(c_[4:4 + 2 * n_]); found = 0
        for i in range(n_):
            for side in (-t / 2, t / 2):
                p_ = np.array([x0, y0]) + u * pts[2 * i] + nlv * side
                found += bool(np.any(np.all(np.abs(v - np.array([p_[0], p_[1], base_of_chain[ci] + pts[2 * i + 1]])) < 1e-6, axis=1)))
        chk(f"{nm}: wall has vertices at every corner of the {n_}-point arched cut starting at s={pts[0]:.3f}", (found, 2 * n_), (2 * n_, 2 * n_))

bad = 0
for ok, name, a, b in checks:
    bad += not ok
    print(f"{'PASS' if ok else 'FAIL'}  {name}" + ("" if ok else f":  {a} vs {b}"))
print(f"{len(checks)} checks, {bad} failed; largest wall volume difference {worst:.2e} m3; file bytes {os.path.getsize(path)}")
sys.exit(1 if bad else 0)
