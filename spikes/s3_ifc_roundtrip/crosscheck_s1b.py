"""Independent checks for S1b.
 1. shapely: wall quads of a floor plan (mitres + T-junctions) must not overlap and must tile exactly.
 2. IfcOpenShell / OpenCASCADE: volume of a gable wall with an arched door and round and square windows,
    built with boolean differences, must equal the Rust result.
Reads /tmp/scene.txt written by `cargo run --release --example scene`."""
import sys, math, numpy as np
import ifcopenshell, ifcopenshell.geom
from shapely.geometry import Polygon
from shapely.ops import unary_union

EXPECT_POLYS, EXPECT_CHAINS = 10, 4
polys, chains, feature = [], [], None
for line in open("/tmp/scene.txt"):
    k, *v = line.split()
    if k == "POLY": v = list(map(float, v)); polys.append(Polygon(list(zip(v[0:8:2], v[1:8:2]))))
    elif k == "CHAIN": chains.append((float(v[0]), float(v[1]), v[2] == "true"))
    elif k == "FEATURE": feature = (float(v[0]), float(v[1]))
if len(polys) != EXPECT_POLYS or len(chains) != EXPECT_CHAINS or feature is None:
    sys.exit(f"scene file incomplete: {len(polys)} quads, {len(chains)} chains, feature={feature}")

fail = 0
def check(name, ok, detail=""):
    global fail; fail += not ok; print(("PASS " if ok else "FAIL ") + name, detail)

check("every wall quad is a valid polygon", all(p.is_valid and p.area > 0 for p in polys), f"{len(polys)} quads")
worst = max(polys[i].intersection(polys[j]).area for i in range(len(polys)) for j in range(i + 1, len(polys)))
check("no two wall quads overlap", worst < 1e-9, f"largest overlap {worst:.2e} m2")
tot, uni = sum(p.area for p in polys), unary_union(polys).area
check("union area equals sum of areas", abs(tot - uni) < 1e-9, f"{tot:.6f} vs {uni:.6f}")
touch = sum(1 for i in range(len(polys)) for j in range(i + 1, len(polys)) if polys[i].distance(polys[j]) < 1e-9)
check("neighbouring walls actually touch (no gaps at joints)", touch >= 7, f"{touch} touching pairs")
check("every chain is one closed manifold shell", all(c[2] for c in chains))
check("exact volume equals mesh volume for every chain", all(abs(a - b) < 1e-9 for a, b, _ in chains))

def arch(x0, w, spring, n):
    r = w / 2; v = [(x0, 0), (x0 + w, 0), (x0 + w, spring)]
    v += [(x0 + r + r * math.cos(math.pi * k / n), spring + r * math.sin(math.pi * k / n)) for k in range(1, n)]
    return v + [(x0, spring)]
def circle(cx, cz, r, n): return [(cx + r * math.cos(math.tau * k / n), cz + r * math.sin(math.tau * k / n)) for k in range(n)]

f = ifcopenshell.file(schema="IFC4")
ctx = f.createIfcGeometricRepresentationContext(None, "Model", 3, 1e-6, f.createIfcAxis2Placement3D(f.createIfcCartesianPoint((0., 0., 0.))))
body = f.createIfcGeometricRepresentationSubContext("Body", "Model", None, None, None, None, ctx, None, "MODEL_VIEW", None)
def solid(pts, depth):
    ring = [f.createIfcCartesianPoint((float(x), float(y))) for x, y in pts]
    prof = f.createIfcArbitraryClosedProfileDef("AREA", None, f.createIfcPolyline(ring + [ring[0]]))
    place = f.createIfcAxis2Placement3D(f.createIfcCartesianPoint((0., 0., 0.)))
    return f.createIfcExtrudedAreaSolid(prof, place, f.createIfcDirection((0., 0., 1.)), depth)

res = solid([(0, 0), (8, 0), (8, 3), (4, 4.5), (0, 3)], 0.25)
for hole in [arch(1.5, 1.2, 1.9, 32), circle(5.5, 2.0, 0.6, 40), [(3, 1), (4, 1), (4, 2), (3, 2)]]:
    res = f.createIfcBooleanResult("DIFFERENCE", res, solid(hole, 0.25))
rep = f.createIfcShapeRepresentation(body, "Body", "Clipping", [res])
wall = f.createIfcWall(ifcopenshell.guid.new(), None, "w", None, None,
                       f.createIfcLocalPlacement(None, f.createIfcAxis2Placement3D(f.createIfcCartesianPoint((0., 0., 0.)))),
                       f.createIfcProductDefinitionShape(None, None, [rep]))
s = ifcopenshell.geom.create_shape(ifcopenshell.geom.settings(), wall)
v = np.array(s.geometry.verts).reshape(-1, 3); t = np.array(s.geometry.faces).reshape(-1, 3)
v_occ = abs(np.einsum("ij,ij->i", v[t[:, 0]], np.cross(v[t[:, 1]], v[t[:, 2]])).sum() / 6)
hand = (Polygon([(0, 0), (8, 0), (8, 3), (4, 4.5), (0, 3)]).area - Polygon(arch(1.5, 1.2, 1.9, 32)).area
        - Polygon(circle(5.5, 2.0, 0.6, 40)).area - 1.0) * 0.25
check("feature wall: Rust exact equals Rust mesh", abs(feature[0] - feature[1]) < 1e-9, f"{feature[0]:.6f} {feature[1]:.6f}")
check("feature wall: Rust equals OpenCASCADE boolean", abs(v_occ - feature[0]) < 1e-4, f"OCC {v_occ:.6f}  Rust {feature[0]:.6f}  hand {hand:.6f}")
check("feature wall: hand calculation equals Rust", abs(hand - feature[0]) < 1e-9)
sys.exit(1 if fail else 0)
