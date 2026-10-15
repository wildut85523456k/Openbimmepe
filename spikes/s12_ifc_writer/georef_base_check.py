"""Georeferencing against the chosen coordinate base (Revit's: internal origin, project base point, survey point, shared coordinates).

usage: georef_base_check.py MANIFEST.txt FILE.ifc BASE      (BASE is one of internal, pbp, survey, shared)

Independent of the Rust code: the file's map conversion is applied with IfcOpenShell's own helper to the position a
slab corner has in the file's mesh, and must give the shared coordinates the model says that corner has; and the
survey point, put through the same conversion, must give the survey point's shared coordinates.
"""
import math, sys
import numpy as np
import ifcopenshell, ifcopenshell.geom
import ifcopenshell.util.geolocation as geo

man = [l.split() for l in open(sys.argv[1]).read().splitlines() if l.strip()]
f = ifcopenshell.open(sys.argv[2])
base = sys.argv[3]
fails = checks = 0
def ok(c, msg):
    global fails, checks
    checks += 1
    if not c:
        fails += 1
        print("FAIL", msg)
rows = lambda tag: [r[1:] for r in man if r[0] == tag]

mc = f.by_type("IfcMapConversion")[0]
st = ifcopenshell.geom.settings(); st.set("use-world-coords", True)
slab = f.by_type("IfcSlab")[0]
v = np.array(ifcopenshell.geom.create_shape(st, slab).geometry.verts).reshape(-1, 3)
sp = [float(x) for x in rows("GEOPOINTS")[0]]
survey_model, survey_shared = np.array(sp[0:3]), np.array(sp[3:6])
ok(len(f.by_type("IfcProjectedCRS")) == 1, "one projected CRS")
scale = mc.Scale
for r in rows("GEOPT"):
    x = [float(t) for t in r]
    model, shared, filept = np.array(x[0:3]), np.array(x[3:6]), np.array(x[9:12])
    e, n, h = geo.xyz2enh(*filept, mc.Eastings, mc.Northings, mc.OrthogonalHeight, mc.XAxisAbscissa, mc.XAxisOrdinate, mc.Scale)
    ok(np.allclose([e, n, h], shared, atol=1e-5), f"{base}: model point {model} is at {np.round([e, n, h], 4)} through the file's map conversion, the model says {np.round(shared, 4)}")
    # the slab's top corners (z = 3.0 in the model) are vertices of the mesh at the file position
    if abs(model[2] - 3.0) < 1e-9 and model[1] > 7.9:
        ok(np.min(np.linalg.norm(v - filept, axis=1)) < 1e-5, f"{base}: slab corner {model} is at {np.round(filept, 4)} in the file's mesh")
# the survey point is where the surveyor says it is: its place in the file comes from the site's own placement
import ifcopenshell.util.placement as up
m = up.get_local_placement(f.by_type("IfcSite")[0].ObjectPlacement)
fp = (m @ np.append(survey_model, 1.0))[:3]
e, n, h = geo.xyz2enh(*fp, mc.Eastings, mc.Northings, mc.OrthogonalHeight, mc.XAxisAbscissa, mc.XAxisOrdinate, mc.Scale)
ok(np.allclose([e, n, h], survey_shared, atol=1e-5), f"{base}: the survey point comes out at {np.round([e, n, h], 4)}, the surveyor gave {survey_shared}")
if base == "survey":
    ok(np.allclose(fp, 0, atol=1e-9), "the survey point is the file's origin")
if base == "pbp":
    ok(np.allclose((m @ np.array([*sp[6:9], 1.0]))[:3], 0, atol=1e-9), "the project base point is the file's origin")
tn = geo.get_true_north(f)
ok(abs(tn - (0.0 if base == "shared" else -12.5)) < 1e-9, f"{base}: true north in the file is {tn}")
print(f"{checks} checks, {fails} failures")
sys.exit(1 if fails else 0)
