"""Independent check of ceilings and shafts: floor slabs, a ceiling and two risers are written to IFC4 (IfcSlab voided by
IfcOpeningElement, IfcCovering), read back with IfcOpenShell / OpenCASCADE, and compared with figures calculated by hand here."""
import sys, collections, tempfile, os
import numpy as np
import ifcopenshell, ifcopenshell.api, ifcopenshell.geom, ifcopenshell.util.element as E

api = ifcopenshell.api.run
by = collections.defaultdict(list)
for l in open(sys.argv[1]).read().strip().split("\n"):
    l = l.split(); by[l[0]].append(l)
if not by["END"]: sys.exit("scene file is incomplete (no END line)")
fl = lambda xs: [float(x) for x in xs]

f = api("project.create_file", version="IFC4")
api("root.create_entity", f, ifc_class="IfcProject", name="S7")
api("unit.assign_unit", f, length={"is_metric": True, "raw": "METERS"})
ctx = api("context.add_context", f, context_type="Model")
body = api("context.add_context", f, context_type="Model", context_identifier="Body", target_view="MODEL_VIEW", parent=ctx)
levels = [(l[1], float(l[2])) for l in by["LEVEL"]]
def ring(pts):
    c = [f.createIfcCartesianPoint(tuple(pts[2 * i:2 * i + 2])) for i in range(len(pts) // 2)]
    return f.createIfcPolyline(c + [c[0]])
def extrude(el, pts, z0, depth):
    prof = f.createIfcArbitraryClosedProfileDef("AREA", None, ring(pts))
    rep = api("geometry.add_profile_representation", f, context=body, profile=prof, depth=depth, cardinal_point=None)
    api("geometry.assign_representation", f, product=el, representation=rep)
    m = np.eye(4); m[2, 3] = z0; api("geometry.edit_object_placement", f, product=el, matrix=m)

sl = by["SLAB"][0]; sl_t = float(sl[1]); sl_pts = fl(sl[3:3 + 2 * int(sl[2])])
slabs = {}
for i in range(1, len(levels)):                      # slabs under L1, L2, Roof; top face at the level elevation
    s = api("root.create_entity", f, ifc_class="IfcSlab", name=f"Slab {levels[i][0]}")
    extrude(s, sl_pts, levels[i][1] - sl_t, sl_t); slabs[i] = s
for sh in by["SHAFT"]:
    name, a, b = sh[1], int(sh[2]), int(sh[3]); n = int(sh[5]); pts = fl(sh[6:6 + 2 * n])
    cuts = [int(x) for c in by["CUTS"] if c[1] == name for x in c[2:]]
    for i in cuts:
        op = api("root.create_entity", f, ifc_class="IfcOpeningElement", name=f"{name} through {levels[i][0]}")
        extrude(op, pts, levels[i][1] - sl_t - 0.1, sl_t + 0.2)
        api("feature.add_feature", f, feature=op, element=slabs[i])
cl = by["CEILING"][0]; c_h, c_t, c_void, c_area = float(cl[2]), float(cl[3]), float(cl[4]), float(cl[5])
ceil_el = api("root.create_entity", f, ifc_class="IfcCovering", name="Ceiling", predefined_type="CEILING")
extrude(ceil_el, sl_pts, levels[1][1] + c_h, c_t)

path = os.path.join(tempfile.mkdtemp(), "s7.ifc"); f.write(path)
g = ifcopenshell.open(path)
S = ifcopenshell.geom.settings(); S.set("use-world-coords", True)
checks = []
def chk(name, a, b, tol=None):
    ok = (abs(a - b) <= tol) if tol is not None else (a == b)
    checks.append((ok, name, a, b))
def shape(e):
    s = ifcopenshell.geom.create_shape(S, e); v = np.array(s.geometry.verts).reshape(-1, 3); fc = np.array(s.geometry.faces).reshape(-1, 3)
    return v, fc
def volume(v, fc): return abs(sum(np.dot(v[a], np.cross(v[b], v[c])) for a, b, c in fc) / 6.0)

# ---- by hand: room 0.15..11.85 x 0.15..7.85 -> 11.7 x 7.7 = 90.09; risers 0.8 x 0.8 and 0.6 x 0.5
area = 11.7 * 7.7; r1 = 0.8 * 0.8; r2 = 0.6 * 0.5
want = {"Slab L1": (area - r1 - r2) * 0.2, "Slab L2": (area - r1) * 0.2, "Slab Roof": area * 0.2}
for s in g.by_type("IfcSlab"):
    v, fc = shape(s)
    chk(f"{s.Name}: net volume vs hand", volume(v, fc), want[s.Name], 1e-9)
chk("three slabs", len(g.by_type("IfcSlab")), 3)
chk("three openings (Riser_1 cuts L1 and L2, Riser_2 cuts L1 only)", sorted(o.Name for o in g.by_type("IfcOpeningElement")), sorted(["Riser_1 through L1", "Riser_1 through L2", "Riser_2 through L1"]))
chk("Rust shaft areas vs hand", (round(float(by["SHAFT"][0][4]), 9), round(float(by["SHAFT"][1][4]), 9)), (round(r1, 9), round(r2, 9)))
# ceiling: volume and heights by hand; void against the slab above taken from the IFC solids themselves
cv, cf = shape(g.by_type("IfcCovering")[0])
chk("ceiling volume vs hand (area x 0.05)", volume(cv, cf), area * 0.05, 1e-9)
chk("ceiling area passed by Rust vs hand", c_area, area, 1e-9)
chk("ceiling underside at 3.6 + 2.7", round(cv[:, 2].min(), 9), 6.3)
slab_above = [s for s in g.by_type("IfcSlab") if s.Name == "Slab L2"][0]
sv, _ = shape(slab_above)
void_from_ifc = sv[:, 2].min() - cv[:, 2].max()
chk("void between ceiling top and the slab above, from the IFC solids", void_from_ifc, 0.65, 1e-9)
chk("void written by Rust vs hand 3.6 - 0.2 - 2.7 - 0.05", c_void, 0.65, 1e-9)
# the shaft really is open: a probe through the middle of Riser_1 in the L1 slab hits no solid
def ray_hits(v, fc, o, d):
    """number of triangles a ray from o along d crosses (Moller-Trumbore)"""
    o = np.array(o, float); d = np.array(d, float); n = 0
    for a, b, c in fc:
        e1, e2 = v[b] - v[a], v[c] - v[a]; p = np.cross(d, e2); det = np.dot(e1, p)
        if abs(det) < 1e-12: continue
        t_ = o - v[a]; u = np.dot(t_, p) / det
        if u < 0 or u > 1: continue
        q = np.cross(t_, e1); w = np.dot(d, q) / det
        if w < 0 or u + w > 1: continue
        if np.dot(e2, q) / det > 0: n += 1
    return n
sl1 = shape([x for x in g.by_type("IfcSlab") if x.Name == "Slab L1"][0])
chk("a vertical ray through Riser_1 meets nothing in the L1 slab", ray_hits(*sl1, (10.4, 2.4, 3.0), (0, 0, 1)), 0)
chk("a vertical ray through Riser_2 meets nothing in the L1 slab", ray_hits(*sl1, (10.3, 5.25, 3.0), (0, 0, 1)), 0)
chk("a vertical ray away from the risers crosses the L1 slab twice (bottom and top)", ray_hits(*sl1, (5.0, 4.0, 3.0), (0, 0, 1)), 2)
sl2 = shape([x for x in g.by_type("IfcSlab") if x.Name == "Slab L2"][0])
chk("Riser_2 stops below L2: a ray at its position crosses the L2 slab twice", ray_hits(*sl2, (10.3, 5.25, 6.5), (0, 0, 1)), 2)
chk("Riser_1 goes through L2: a ray at its position meets nothing", ray_hits(*sl2, (10.4, 2.4, 6.5), (0, 0, 1)), 0)

bad = 0
for ok, name, a, b in checks:
    bad += not ok
    print(f"{'PASS' if ok else 'FAIL'}  {name}" + ("" if ok else f":  {a} vs {b}"))
print(f"{len(checks)} checks, {bad} failed")
sys.exit(1 if bad else 0)
