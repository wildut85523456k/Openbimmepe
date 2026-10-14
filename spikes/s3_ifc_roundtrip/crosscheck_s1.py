"""Independent check for S1: IfcOpenShell's own boolean volume for a wall with a door and a window."""
import ifcopenshell, ifcopenshell.api, ifcopenshell.geom, numpy as np, sys
api = ifcopenshell.api.run
f = api("project.create_file", version="IFC4")
api("root.create_entity", f, ifc_class="IfcProject", name="x"); api("unit.assign_unit", f)
ctx = api("context.add_context", f, context_type="Model")
body = api("context.add_context", f, context_type="Model", context_identifier="Body", target_view="MODEL_VIEW", parent=ctx)
wall = api("root.create_entity", f, ifc_class="IfcWall", name="W")
api("geometry.assign_representation", f, product=wall, representation=api("geometry.add_wall_representation", f, context=body, length=6.0, height=3.0, thickness=0.2))
def hole(x, z, w, h):
    o = api("root.create_entity", f, ifc_class="IfcOpeningElement")
    api("geometry.assign_representation", f, product=o, representation=api("geometry.add_wall_representation", f, context=body, length=w, height=h, thickness=0.6))
    m = np.eye(4); m[:3, 3] = (x, -0.2, z); api("geometry.edit_object_placement", f, product=o, matrix=m)
    api("feature.add_feature", f, feature=o, element=wall)
hole(2.0, 0.0, 0.9, 2.1); hole(4.0, 0.9, 1.2, 1.2)
s = ifcopenshell.geom.settings()
sh = ifcopenshell.geom.create_shape(s, wall)
v = np.array(sh.geometry.verts).reshape(-1, 3); t = np.array(sh.geometry.faces).reshape(-1, 3)
vol = abs(np.einsum("ij,ij->i", v[t[:,0]], np.cross(v[t[:,1]], v[t[:,2]])).sum() / 6)
exact, mesh = map(float, open("/tmp/rust_vol.txt").read().split())
print(f"IfcOpenShell {vol:.6f}  Rust exact {exact:.6f}  Rust mesh {mesh:.6f}")
ok = abs(vol - exact) < 1e-6 and abs(vol - mesh) < 1e-6
print("PASS" if ok else "FAIL"); sys.exit(0 if ok else 1)
