"""Independent check of extrusions, revolves and sweeps (S2g).

Reads the output of `cargo run --example forms_scene` and rebuilds every form on its own:
 * extrusions and revolves as IfcExtrudedAreaSolid / IfcRevolvedAreaSolid, meshed by OpenCASCADE through IfcOpenShell;
 * mitred sweeps from scratch: offset lines are intersected at every corner, each piece between two mitre planes is the
   convex hull (scipy) of its two end rings, holes are subtracted the same way (IfcOpenShell's polyline sweep does not mitre).
Volumes and bounding boxes are compared; negative controls prove the comparison can fail.
"""
import math, sys
import numpy as np
import ifcopenshell, ifcopenshell.api, ifcopenshell.geom
import ifcopenshell.util.shape as us
from scipy.spatial import ConvexHull

text = open(sys.argv[1]).read() if len(sys.argv) > 1 else sys.stdin.read()
rust = {}
for l in text.splitlines():
    t = l.split()
    if t and t[0] == "FORM":
        rust[t[1]] = (float(t[2]), np.array(list(map(float, t[3:9]))), t[9] == "true")
checks = fails = 0
def ok(c, m):
    global checks, fails
    checks += 1
    if not c:
        fails += 1; print("FAIL", m)

def rectpts(u0, v0, u1, v1): return [(u0, v0), (u1, v0), (u1, v1), (u0, v1)]
LSHAPE = [(0, 0), (0, 2), (1, 2), (1, 1), (2, 1), (2, 0)]

# ---- profiles: ("poly", pts) | ("circle", cu, cv, r); each with a list of holes
P = lambda outer, *holes: (outer, list(holes))
poly = lambda pts: ("poly", pts)
circ = lambda cu, cv, r: ("circle", cu, cv, r)
rect = lambda *a: poly(rectpts(*a))

EXT = {
 "E1": ("Z", 0, 3, P(rect(0, 0, .4, .2))),
 "E2": ("Z", 1, 1.5, P(poly(LSHAPE))),
 "E3": ("X", 5, 8, P(circ(1, 2, .05), circ(1, 2, .04))),
 "E4": ("Y", 7, 8, P(rect(0, 3, 2, 5), circ(1, 4, .5))),
 "E5": ("Z", 0, 1, P(rect(0, 0, 4, 2), rect(.5, .5, 1.5, 1.5), circ(3, 1, .5))),
}
REV = {}
for a in (360, 180, 90, 270, 45, 200, 30): REV[f"R{a}"] = ("Z", 0, True, 0, a, P(rect(0, 0, .5, 2)))
REV["Rtorus"] = ("Z", 0, True, 0, 360, P(circ(1, 0, .1)))
REV["Rtorus120"] = ("Z", 0, True, 0, 120, P(circ(1, 0, .1)))
REV["Ru"] = ("Z", 0, False, 0, 360, P(rect(0, 1, 3, 2)))
REV["Ru100"] = ("Z", .5, False, -1, 100, P(rect(0, 0, 3, 2)))
REV["Rx"] = ("X", 4, True, 0, 360, P(rect(0, 0, 1, 2)))
REV["Rx60"] = ("X", 4, True, .5, 60, P(rect(1, 0, 2, 2)))
REV["Ry"] = ("Y", 9, True, 0, 300, P(rect(0, 0, 1, 2)))
REV["Rcone"] = ("Z", 0, True, 0, 360, P(poly([(0, 0), (.5, 0), (0, 1)])))
REV["Rhole"] = ("Z", 0, True, 0, 250, P(rect(.2, 0, 1, 2), rect(.4, .5, .8, 1.5)))

L = [(0, 0), (4, 0), (4, 3)]
SW = {
 "S1": (L, rect(-.1, -.05, .1, .05), None),
 "S2": (L, rect(0, -.05, .2, .05), None),
 "S3": (L, rect(-.2, -.05, 0, .05), None),
 "S4": ([(0, 0), (4, 0), (4, -3)], rect(0, -.05, .2, .05), None),
 "S5": ([(0, 0), (4, 0), (4, 3), (1, 3)], rect(0, -.05, .2, .05), None),
 "S6": ([(0, 0), (4, 0), (4, 3), (8, 3)], rect(0, -.05, .2, .05), None),
 "S7": ([(0, 0), (4, 0), (6, 2)], rect(0, -.05, .2, .05), None),
 "S8": ([(0, 0), (5, 0), (7, 1), (8, 4), (3, 6)], rect(-.1, 0, .3, .2), None),
 "S9": ([(0, 0), (3, 0), (3, 2)], rect(.02, -.02, .3, .04), None),
 "S10": ([(0, 0), (3, 0), (2, 1.5), (-2, 1.2)], rect(-.15, -.05, .1, .2), None),
 "S11": (L, rect(-.1, -.1, .1, .1), rect(.02, -.02, .08, .02)),
}
SWZ = 1.0

# ---------------- IFC construction
def build(profile, kind, plane, off, **kw):
    f = ifcopenshell.file(schema="IFC4")
    run = ifcopenshell.api.run
    run("root.create_entity", f, ifc_class="IfcProject", name="p")
    run("unit.assign_unit", f, units=[run("unit.add_si_unit", f, unit_type="LENGTHUNIT"), run("unit.add_si_unit", f, unit_type="PLANEANGLEUNIT")])
    el = run("root.create_entity", f, ifc_class="IfcBuildingElementProxy", name="x")
    pt = lambda *c: f.create_entity("IfcCartesianPoint", Coordinates=tuple(float(x) for x in c))
    dr = lambda *c: f.create_entity("IfcDirection", DirectionRatios=tuple(float(x) for x in c))
    def curve(c):
        if c[0] == "poly":
            pts = [pt(*p) for p in c[1]]; pts.append(pts[0])
            return f.create_entity("IfcPolyline", Points=pts)
        return f.create_entity("IfcCircle", Position=f.create_entity("IfcAxis2Placement2D", Location=pt(c[1], c[2])), Radius=float(c[3]))
    outer, holes = profile
    if outer[0] == "circle" and len(holes) == 1 and holes[0][0] == "circle" and holes[0][1:3] == outer[1:3]:
        prof = f.create_entity("IfcCircleHollowProfileDef", ProfileType="AREA", Position=f.create_entity("IfcAxis2Placement2D", Location=pt(outer[1], outer[2])), Radius=float(outer[3]), WallThickness=float(outer[3] - holes[0][3]))
    elif holes:
        prof = f.create_entity("IfcArbitraryProfileDefWithVoids", ProfileType="AREA", OuterCurve=curve(outer), InnerCurves=[curve(h) for h in holes])
    else:
        prof = f.create_entity("IfcArbitraryClosedProfileDef", ProfileType="AREA", OuterCurve=curve(outer))
    axis, ref, o = {"Z": ((0, 0, 1), (1, 0, 0), (0, 0, off)), "X": ((1, 0, 0), (0, 1, 0), (off, 0, 0)), "Y": ((0, 1, 0), (0, 0, 1), (0, off, 0))}[plane]
    pos = f.create_entity("IfcAxis2Placement3D", Location=pt(*o), Axis=dr(*axis), RefDirection=dr(*ref))
    if kind == "ext":
        solid = f.create_entity("IfcExtrudedAreaSolid", SweptArea=prof, Position=pos, ExtrudedDirection=dr(0, 0, 1), Depth=float(kw["depth"]))
    else:
        loc, d = ((kw["at"], 0, 0), (0, 1, 0)) if kw["about_v"] else ((0, kw["at"], 0), (1, 0, 0))
        ax = f.create_entity("IfcAxis1Placement", Location=pt(*loc), Axis=dr(*d))
        solid = f.create_entity("IfcRevolvedAreaSolid", SweptArea=prof, Position=pos, Axis=ax, Angle=math.radians(kw["angle"]))
    ctx = run("context.add_context", f, context_type="Model")
    sub = run("context.add_context", f, context_type="Model", context_identifier="Body", target_view="MODEL_VIEW", parent=ctx)
    rep = f.create_entity("IfcShapeRepresentation", ContextOfItems=sub, RepresentationIdentifier="Body", RepresentationType="SweptSolid", Items=[solid])
    el.Representation = f.create_entity("IfcProductDefinitionShape", Representations=[rep])
    s = ifcopenshell.geom.settings()
    s.set("mesher-linear-deflection", 2e-5 if kind == "ext" else 1e-4); s.set("mesher-angular-deflection", 0.02 if kind == "ext" else 0.1)
    g = ifcopenshell.geom.create_shape(s, el).geometry
    v = np.array(g.verts).reshape(-1, 3)
    return us.get_volume(g), np.concatenate([v.min(0), v.max(0)])

def curved(p): return any(c[0] == "circle" for c in [p[0]] + p[1])

# ---------------- independent mitred sweep
def sweep(path, outer, hole):
    path = [np.array(p, float) for p in path]
    n = len(path)
    d = [(path[i + 1] - path[i]) / np.linalg.norm(path[i + 1] - path[i]) for i in range(n - 1)]
    left = [np.array([-x[1], x[0]]) for x in d]
    def ring(u, v):
        pts = []
        for i in range(n):
            if i == 0: q = path[0] + u * left[0]
            elif i == n - 1: q = path[i] + u * left[-1]
            else:  # intersection of the two offset lines
                a = path[i] + u * left[i - 1]; b = path[i] + u * left[i]
                A = np.array([d[i - 1], -d[i]]).T
                t = np.linalg.solve(A, b - a)
                q = a + t[0] * d[i - 1]
            pts.append(np.array([q[0], q[1], SWZ + v]))
        return pts
    def solid(prof):
        vol = 0.0; allp = []
        rings = [ring(u, v) for (u, v) in prof]
        for i in range(n - 1):
            P = np.array([r[i] for r in rings] + [r[i + 1] for r in rings])
            vol += ConvexHull(P).volume; allp.append(P)
        return vol, np.vstack(allp)
    v1, pts = solid(outer[1])
    if hole: v1 -= solid(hole[1])[0]
    return v1, np.concatenate([pts.min(0), pts.max(0)])

def compare(name, vol, box, curved_case, exact_flag=None):
    rv, rb, ex = rust[name]
    tv = 1e-3 * max(1, abs(rv)) if curved_case else 1e-9 * max(1, abs(rv))
    tb = 1e-3 if curved_case else 1e-9
    ok(abs(vol - rv) <= tv, f"{name} volume rust {rv} independent {vol}")
    ok(np.allclose(box, rb, atol=tb), f"{name} bounds rust {rb} independent {box}")
    ok(ex, f"{name} bounds reported exact")

results = {}
for name, (pl, a, b, prof) in EXT.items():
    results[name] = build(prof, "ext", pl, a, depth=b - a)
    compare(name, *results[name], curved(prof))
for name, (pl, off, av, at, ang, prof) in REV.items():
    results[name] = build(prof, "rev", pl, off, about_v=av, at=at, angle=ang)
    compare(name, *results[name], True)
for name, (path, outer, hole) in SW.items():
    results[name] = sweep(path, outer, hole)
    compare(name, *results[name], False)

# ---- the closed forms behind a few cases (third, formula-based opinion)
ok(abs(results["Rtorus"][0] - 2 * math.pi ** 2 * 1 * .01) < 1e-4, "torus formula")
ok(abs(results["S1"][0] - .14) < 1e-12 and abs(results["S2"][0] - .136) < 1e-12 and abs(results["S5"][0] - .192) < 1e-12, "sweep sample values")

# ---- negative controls: a wrong figure must be reported
before = fails
for name in ("E4", "R180", "S2"):
    rv, rb, ex = rust[name]
    c = fails
    for bad_v, bad_b in ((rv * 1.01, rb), (rv, rb + np.array([0, 0, 0, 0, 0.01, 0]))):
        tv = 1e-3 * max(1, rv)
        detected = abs(results[name][0] - bad_v) > tv or not np.allclose(results[name][1], bad_b, atol=1e-3)
        ok(detected, f"negative control {name} not detected")
# the mitre matters: a straight-cut (non-mitred) L would give a different volume
ok(abs(results["S2"][0] - 0.2 * 0.1 * 7) > 1e-3, "mitre control")

print(f"{checks} checks, {fails} failures")
sys.exit(1 if fails else 0)
