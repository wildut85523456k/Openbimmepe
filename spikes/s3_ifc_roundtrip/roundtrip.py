"""Spike S3: can we author a small building in IFC4 and read it back unchanged?

Authors: site > building > storey, 4 walls, a slab, a door with its opening, one space.
Writes IFC4, re-opens it, and checks counts, names, relationships and geometry.
"""
import sys, tempfile, os
import ifcopenshell, ifcopenshell.api, ifcopenshell.geom, ifcopenshell.util.element as E
import numpy as np

api = ifcopenshell.api.run
f = api("project.create_file", version="IFC4")
proj = api("root.create_entity", f, ifc_class="IfcProject", name="S3 spike")
api("unit.assign_unit", f)
ctx = api("context.add_context", f, context_type="Model")
body = api("context.add_context", f, context_type="Model", context_identifier="Body",
           target_view="MODEL_VIEW", parent=ctx)
site = api("root.create_entity", f, ifc_class="IfcSite", name="Site")
bld = api("root.create_entity", f, ifc_class="IfcBuilding", name="Building")
st = api("root.create_entity", f, ifc_class="IfcBuildingStorey", name="Level 0")
api("aggregate.assign_object", f, products=[site], relating_object=proj)
api("aggregate.assign_object", f, products=[bld], relating_object=site)
api("aggregate.assign_object", f, products=[st], relating_object=bld)

W, D, H, T = 6.0, 4.0, 3.0, 0.2
def place(el, x=0, y=0, z=0):
    m = np.eye(4); m[:3, 3] = (x, y, z)
    api("geometry.edit_object_placement", f, product=el, matrix=m)

walls = []
for i, (x, y, length, ang) in enumerate([(0,0,W,0),(W,0,D,90),(W,D,W,180),(0,D,D,270)]):
    w = api("root.create_entity", f, ifc_class="IfcWall", name=f"Wall {i+1}")
    rep = api("geometry.add_wall_representation", f, context=body, length=length, height=H, thickness=T)
    api("geometry.assign_representation", f, product=w, representation=rep)
    c, s = np.cos(np.radians(ang)), np.sin(np.radians(ang))
    m = np.array([[c,-s,0,x],[s,c,0,y],[0,0,1,0],[0,0,0,1]])
    api("geometry.edit_object_placement", f, product=w, matrix=m)
    api("spatial.assign_container", f, products=[w], relating_structure=st)
    walls.append(w)

slab = api("root.create_entity", f, ifc_class="IfcSlab", name="Floor slab", predefined_type="FLOOR")
rep = api("geometry.add_slab_representation", f, context=body, depth=0.25)
api("geometry.assign_representation", f, product=slab, representation=rep)
api("spatial.assign_container", f, products=[slab], relating_structure=st)

door = api("root.create_entity", f, ifc_class="IfcDoor", name="Door 1", predefined_type="DOOR")
door.OverallWidth, door.OverallHeight = 0.9, 2.1
place(door, 2.0, 0, 0)
opening = api("root.create_entity", f, ifc_class="IfcOpeningElement", name="Opening Door 1")
place(opening, 2.0, -0.05, 0)
api("feature.add_feature", f, feature=opening, element=walls[0])
api("feature.add_filling", f, opening=opening, element=door)
api("spatial.assign_container", f, products=[door], relating_structure=st)

space = api("root.create_entity", f, ifc_class="IfcSpace", name="Room 1")
api("aggregate.assign_object", f, products=[space], relating_object=st)

path = os.path.join(tempfile.mkdtemp(), "spike.ifc")
f.write(path)
g = ifcopenshell.open(path)

def count(m, c): return len(m.by_type(c))
checks = []
for c in ["IfcWall", "IfcSlab", "IfcDoor", "IfcOpeningElement", "IfcSpace", "IfcBuildingStorey"]:
    checks.append((f"count {c}", count(f, c), count(g, c)))
checks.append(("schema", f.schema, g.schema))
checks.append(("wall names", sorted(w.Name for w in f.by_type("IfcWall")), sorted(w.Name for w in g.by_type("IfcWall"))))
d2 = g.by_type("IfcDoor")[0]
checks.append(("door in wall opening", True,
               any(r.RelatingBuildingElement.is_a("IfcWall") for o in d2.FillsVoids for r in o.RelatingOpeningElement.VoidsElements)))
checks.append(("walls in storey", 4, sum(1 for w in g.by_type("IfcWall") if E.get_container(w) and E.get_container(w).Name == "Level 0")))

settings = ifcopenshell.geom.settings()
def vol(model):
    tot = 0.0
    for w in model.by_type("IfcWall"):
        sh = ifcopenshell.geom.create_shape(settings, w)
        v = np.array(sh.geometry.verts).reshape(-1, 3); fcs = np.array(sh.geometry.faces).reshape(-1, 3)
        a, b, c = v[fcs[:,0]], v[fcs[:,1]], v[fcs[:,2]]
        tot += abs(np.einsum("ij,ij->i", a, np.cross(b, c)).sum() / 6)
    return round(tot, 4)
checks.append(("wall volume m3", vol(f), vol(g)))

bad = 0
for name, a, b in checks:
    ok = a == b; bad += not ok
    print(f"{'PASS' if ok else 'FAIL'}  {name}: {a} vs {b}")
print("file bytes:", os.path.getsize(path))
sys.exit(1 if bad else 0)
