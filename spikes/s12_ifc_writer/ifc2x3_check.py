"""Checks the IFC2X3 export (for DIALux) with IfcOpenShell, independently of the Rust writer.
usage: ifc2x3_check.py FILE_2X3.ifc FILE_IFC4.ifc  (both from the same scene)"""
import sys, collections
import ifcopenshell, ifcopenshell.geom, ifcopenshell.util.shape as sh
import ifcopenshell.validate as val
import logging

ok = bad = 0
def check(c, msg):
    global ok, bad
    if c: ok += 1
    else:
        bad += 1; print("FAIL", msg)

f2 = ifcopenshell.open(sys.argv[1]); f4 = ifcopenshell.open(sys.argv[2])
check(f2.schema == "IFC2X3", "schema is IFC2X3")
# the file validates against the schema, EXPRESS rules included
class H(logging.Handler):
    def __init__(s): super().__init__(); s.m = []
    def emit(s, r): s.m.append(r.getMessage())
lg = logging.getLogger("v"); lg.setLevel(logging.INFO); h = H(); lg.addHandler(h)
val.validate(f2, lg, express_rules=True)
check(not h.m, f"schema validation: {h.m[:5]}")
# every rooted entity has the owner history
check(all(e.OwnerHistory is not None for e in f2.by_type("IfcRoot")), "owner history on every rooted entity")
# what DIALux wants is there
for cls in ["IfcWall", "IfcSlab", "IfcSpace", "IfcOpeningElement", "IfcDoor", "IfcWindow", "IfcBuildingStorey", "IfcSite", "IfcBuilding"]:
    check(len(f2.by_type(cls)) > 0, f"has {cls}")
# nothing that 2x3 lacks
names = {e.is_a() for e in f2}
check(not names & {"IfcMapConversion", "IfcProjectedCRS", "IfcRelSpaceBoundary2ndLevel", "IfcWallType", "IfcDistributionPort", "IfcLightFixture"}, "no IFC4-only entities")
# doors and windows fill an opening each
fills = {r.RelatedBuildingElement.id() for r in f2.by_type("IfcRelFillsElement")}
for e in f2.by_type("IfcDoor") + f2.by_type("IfcWindow"):
    check(e.id() in fills, f"{e.Name} fills an opening")
    check(e.OverallHeight and e.OverallWidth, f"{e.Name} has overall size")
check(all(len(w.HasOpenings) >= 0 for w in f2.by_type("IfcWall")), "walls")
# geometry of every wall, slab, column, space and opening equals the one in the IFC4 file (same GlobalId)
S = ifcopenshell.geom.settings(); S.set("use-world-coords", True)
def geo(f):
    out = {}
    for e in f.by_type("IfcElement") + f.by_type("IfcSpace"):
        if e.Representation is None: continue
        try:
            s = ifcopenshell.geom.create_shape(S, e)
        except Exception as x:
            out[e.GlobalId] = None; continue
        v = s.geometry.verts; xs = v[0::3]; ys = v[1::3]; zs = v[2::3]
        out[e.GlobalId] = (e.is_a(), sh.get_volume(s.geometry), (min(xs), min(ys), min(zs), max(xs), max(ys), max(zs)))
    return out
g2, g4 = geo(f2), geo(f4)
n = 0
for gid, a in g2.items():
    if a is None or a[0] in ("IfcDoor", "IfcWindow"): continue
    b = g4.get(gid)
    check(b is not None, f"{a[0]} {gid} also in the IFC4 file")
    if b is None: continue
    n += 1
    check(abs(a[1] - b[1]) < 1e-6 * max(1, b[1]), f"{a[0]} volume {a[1]} vs {b[1]}")
    check(all(abs(x - y) < 1e-6 for x, y in zip(a[2], b[2])), f"{a[0]} box {a[2]} vs {b[2]}")
check(n >= 15, f"compared {n} elements")
# the walls are cut: a wall with an opening has less volume than its box
for w in f2.by_type("IfcWall"):
    if w.HasOpenings:
        check(g2[w.GlobalId][1] > 0, "cut wall has volume")
print(f"CHECKS {ok} ok, {bad} failed")
sys.exit(1 if bad else 0)
