"""Independent check of levels and rooms: write them to IFC4 (IfcBuildingStorey, IfcSpace), read them back with
IfcOpenShell / OpenCASCADE, and compare with figures calculated by hand in this file (not with the Rust output alone)."""
import sys, collections, math, tempfile, os
import numpy as np
import ifcopenshell, ifcopenshell.api, ifcopenshell.geom, ifcopenshell.util.element as E

api = ifcopenshell.api.run
by = collections.defaultdict(list)
for l in open(sys.argv[1]).read().strip().split("\n"):
    l = l.split(); by[l[0]].append(l)
if not by["END"]: sys.exit("scene file is incomplete (no END line)")
fl = lambda xs: [float(x) for x in xs]

f = api("project.create_file", version="IFC4")
proj = api("root.create_entity", f, ifc_class="IfcProject", name="S5 rooms")
api("unit.assign_unit", f, length={"is_metric": True, "raw": "METERS"})
ctx = api("context.add_context", f, context_type="Model")
body = api("context.add_context", f, context_type="Model", context_identifier="Body", target_view="MODEL_VIEW", parent=ctx)
site = api("root.create_entity", f, ifc_class="IfcSite", name="Site")
bld = api("root.create_entity", f, ifc_class="IfcBuilding", name="Building")
api("aggregate.assign_object", f, products=[site], relating_object=proj)
api("aggregate.assign_object", f, products=[bld], relating_object=site)
storeys = {}
for l in by["LEVEL"]:
    s = api("root.create_entity", f, ifc_class="IfcBuildingStorey", name=l[1]); s.Elevation = float(l[2])
    api("aggregate.assign_object", f, products=[s], relating_object=bld)
    m = np.eye(4); m[2, 3] = float(l[2]); api("geometry.edit_object_placement", f, product=s, matrix=m)
    storeys[l[1]] = s

def ring(pts): 
    c = [f.createIfcCartesianPoint(tuple(pts[2 * i:2 * i + 2])) for i in range(len(pts) // 2)]
    return f.createIfcPolyline(c + [c[0]])
lvl_name = {i: l[1] for i, l in enumerate(by["LEVEL"])}
outline = {l[1]: fl(l[3:3 + 2 * int(l[2])]) for l in by["OUTLINE"]}
holes = collections.defaultdict(list)
for l in by["HOLE"]: holes[l[1]].append(fl(l[3:3 + 2 * int(l[2])]))
rooms = {}
for l in by["ROOM"]:
    name, usage, occ, lev, elev, h = l[1], l[2], float(l[3]), int(l[4]), float(l[5]), float(l[6])
    sp = api("root.create_entity", f, ifc_class="IfcSpace", name=name)
    sp.LongName = usage
    outer = ring(outline[name])
    prof = f.createIfcArbitraryProfileDefWithVoids("AREA", None, outer, [ring(x) for x in holes[name]]) if holes[name] else f.createIfcArbitraryClosedProfileDef("AREA", None, outer)
    rep = api("geometry.add_profile_representation", f, context=body, profile=prof, depth=h, cardinal_point=None)
    api("geometry.assign_representation", f, product=sp, representation=rep)
    m = np.eye(4); m[2, 3] = elev; api("geometry.edit_object_placement", f, product=sp, matrix=m)
    api("aggregate.assign_object", f, products=[sp], relating_object=storeys[lvl_name[lev]])
    ps = api("pset.add_pset", f, product=sp, name="OpenBIM_Room")
    api("pset.edit_pset", f, pset=ps, properties={"Usage": usage, "Occupancy": occ})
    rooms[name] = sp

path = os.path.join(tempfile.mkdtemp(), "s5.ifc"); f.write(path)
g = ifcopenshell.open(path)
S = ifcopenshell.geom.settings(); S.set("use-world-coords", True)
checks = []
def chk(name, a, b, tol=None):
    ok = (abs(a - b) <= tol) if tol is not None else (a == b)
    checks.append((ok, name, a, b))
def shape(e):
    s = ifcopenshell.geom.create_shape(S, e); v = np.array(s.geometry.verts).reshape(-1, 3); fc = np.array(s.geometry.faces).reshape(-1, 3)
    return v, fc
def volume(v, fc):
    return abs(sum(np.dot(v[a], np.cross(v[b], v[c])) for a, b, c in fc) / 6.0)

# ---- expectations calculated by hand
# Office_1: closed 8 x 5 room, wall 0.2, faces 0.1 inside the centreline -> 7.8 x 4.8, height 3.0, ground level
want = {"Office_1": dict(area=7.8 * 4.8, height=3.0, elev=0.0, occ=6.0)}
# Lobby: L-shaped, wall 0.4 (faces 0.2 inside): bottom strip x 10.2..15.8, y 0.2..1.8; upper block x 10.2..12.8, y 1.8..3.8
#        minus a core of 1 x 1 with walls 0.2 (outer faces 1.2 x 1.2); room height 2.8; first level at 3.5
want["Lobby"] = dict(area=5.6 * 1.6 + 2.6 * 2.0 - 1.2 * 1.2, height=2.8, elev=3.5, occ=10.0)
rust = {l[1]: (float(l[7]), float(l[8])) for l in by["ROOM"]}
for n, w in want.items():
    sp = [s for s in g.by_type("IfcSpace") if s.Name == n][0]
    v, fc = shape(sp)
    chk(f"{n}: volume from the IFC solid vs hand (area x height)", volume(v, fc), w["area"] * w["height"], 1e-9)
    chk(f"{n}: Rust area vs hand", rust[n][0], w["area"], 1e-9)
    chk(f"{n}: Rust volume vs hand", rust[n][1], w["area"] * w["height"], 1e-9)
    chk(f"{n}: bottom of the solid at the level elevation", round(v[:, 2].min(), 9), w["elev"])
    chk(f"{n}: top of the solid is height above it", round(v[:, 2].max(), 9), round(w["elev"] + w["height"], 9))
    chk(f"{n}: occupancy property", E.get_psets(sp)["OpenBIM_Room"]["Occupancy"], w["occ"])
    st = E.get_container(sp) if E.get_container(sp) else sp.Decomposes[0].RelatingObject
    chk(f"{n}: sits in the right storey", st.Elevation, w["elev"])
# the Office solid is a box with the hand-calculated corners; the Lobby core hole must not be filled
v, _ = shape([s for s in g.by_type("IfcSpace") if s.Name == "Office_1"][0])
chk("Office_1: solid spans x 0.1..7.9", (round(v[:, 0].min(), 9), round(v[:, 0].max(), 9)), (0.1, 7.9))
chk("Office_1: solid spans y 0.1..4.9", (round(v[:, 1].min(), 9), round(v[:, 1].max(), 9)), (0.1, 4.9))
chk("two storeys at 0 and 3.5", sorted(s.Elevation for s in g.by_type("IfcBuildingStorey")), [0.0, 3.5])
# grid crossings by hand: grid 1 is x = 0, A is y = 3, B is y = 8
cr = {(l[1], l[2]): fl(l[3:5]) for l in by["CROSS"]}
chk("grid 1 x A cross at (0, 3)", cr[("1", "A")], [0.0, 3.0])
chk("grid 1 x B cross at (0, 8)", cr[("1", "B")], [0.0, 8.0])

bad = 0
for ok, name, a, b in checks:
    bad += not ok
    print(f"{'PASS' if ok else 'FAIL'}  {name}" + ("" if ok else f":  {a} vs {b}"))
print(f"{len(checks)} checks, {bad} failed")
sys.exit(1 if bad else 0)
