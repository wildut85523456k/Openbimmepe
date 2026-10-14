"""Independent check of layered assemblies: write the wall type to IFC4 as an IfcWallType with an
IfcMaterialLayerSet, read it back, recompute U and the thickness from what the file says, and compare with figures
calculated by hand in this script as well as with the Rust output. Envelope areas are compared with hand formulas."""
import sys, collections, math, tempfile, os
import ifcopenshell, ifcopenshell.api, ifcopenshell.util.element as E

api = ifcopenshell.api.run
by = collections.defaultdict(list)
for l in open(sys.argv[1]).read().strip().split("\n"):
    l = l.split(); by[l[0]].append(l)
if not by["END"]: sys.exit("scene file is incomplete (no END line)")

f = api("project.create_file", version="IFC4")
api("root.create_entity", f, ifc_class="IfcProject", name="S6 layers")
api("unit.assign_unit", f, length={"is_metric": True, "raw": "METERS"})
mats = {}
for l in by["MATERIAL"]:
    m = api("material.add_material", f, name=l[1], category="concrete")
    ps = api("pset.add_pset", f, product=m, name="Pset_MaterialThermal")
    api("pset.edit_pset", f, pset=ps, properties={"ThermalConductivity": float(l[2])})
    mats[l[1]] = m
a = by["ASSEMBLY"][0]; r_se, r_si = float(a[2]), float(a[3])
wt = api("root.create_entity", f, ifc_class="IfcWallType", name=a[1], predefined_type="STANDARD")
ls = api("material.add_material_set", f, name=a[1], set_type="IfcMaterialLayerSet")
for l in by["LAYER"]:
    if l[1] == "solid":
        mat, t = mats[l[2]], float(l[3])
    else:
        mat = api("material.add_material", f, name=l[2], category="air"); t = float(l[3])
        ps = api("pset.add_pset", f, product=mat, name="OpenBIM_Layer")
        api("pset.edit_pset", f, pset=ps, properties={"ThermalResistance": float(l[4])})
    layer = api("material.add_layer", f, layer_set=ls, material=mat)
    layer.LayerThickness = t
api("material.assign_material", f, products=[wt], type="IfcMaterialLayerSet", material=ls)
ps = api("pset.add_pset", f, product=wt, name="Pset_WallCommon")
api("pset.edit_pset", f, pset=ps, properties={"ThermalTransmittance": float(by["U"][0][1]), "IsExternal": True})
ps = api("pset.add_pset", f, product=wt, name="OpenBIM_Surfaces")
api("pset.edit_pset", f, pset=ps, properties={"SurfaceResistanceOutside": r_se, "SurfaceResistanceInside": r_si})

path = os.path.join(tempfile.mkdtemp(), "s6.ifc"); f.write(path)
g = ifcopenshell.open(path)
checks = []
def chk(name, a, b, tol=None):
    ok = (abs(a - b) <= tol) if tol is not None else (a == b)
    checks.append((ok, name, a, b))

# ---- read the layers back from the file and recompute
t = g.by_type("IfcWallType")[0]
layers = E.get_material(t).MaterialLayers
chk("five layers in the file", len(layers), 5)
R = E.get_psets(t)["OpenBIM_Surfaces"]["SurfaceResistanceOutside"] + E.get_psets(t)["OpenBIM_Surfaces"]["SurfaceResistanceInside"]
total_t = 0.0
for ly in layers:
    ps = E.get_psets(ly.Material)
    total_t += ly.LayerThickness
    R += ps["OpenBIM_Layer"]["ThermalResistance"] if "OpenBIM_Layer" in ps else ly.LayerThickness / ps["Pset_MaterialThermal"]["ThermalConductivity"]
u_file = 1.0 / R
# ---- by hand: 0.04 + 0.13 + 0.02/0.5 + 0.2/1.0 + 0.05/0.04 + 0.015/0.5 + 0.18 = 1.87
chk("U recomputed from the file vs hand 1/1.87", u_file, 1.0 / 1.87, 1e-12)
chk("U written by Rust vs hand", float(by["U"][0][1]), 1.0 / 1.87, 1e-12)
chk("Pset_WallCommon ThermalTransmittance vs hand", E.get_psets(t)["Pset_WallCommon"]["ThermalTransmittance"], 1.0 / 1.87, 1e-12)
chk("layer thicknesses add to 0.315 m", total_t, 0.02 + 0.2 + 0.05 + 0.015 + 0.03, 1e-12)
chk("Rust thickness vs hand", float(by["THICKNESS"][0][1]), 0.315, 1e-12)
chk("layer order: plaster, block, insulation, plaster, cavity", [ly.Material.Name for ly in layers], ["Plaster", "Block", "Insulation", "Plaster", "Cavity"])

# ---- envelope by hand: office 12 x 8, wall 0.315, height 3.2, north rotated 30 degrees; outside faces 0.1575 beyond the centreline
south = (12 + 0.315) * 3.2; east = (8 + 0.315) * 3.2
env = {int(l[1]): l for l in by["ENV"]}
want = {0: (210.0, "SW", south, 3 * 1.8 * 1.5), 1: (120.0, "SE", east, 1.0 * 2.1), 2: (30.0, "NE", south, 0.0), 3: (300.0, "NW", east, 0.0)}
for k, (az, sec, gross, op) in want.items():
    e = env[k]
    chk(f"segment {k}: azimuth", float(e[2]), az, 1e-9)
    chk(f"segment {k}: sector", e[3], sec)
    chk(f"segment {k}: gross area", float(e[4]), gross, 1e-9)
    chk(f"segment {k}: opening area", float(e[5]), op, 1e-9)
    chk(f"segment {k}: net area", float(e[6]), gross - op, 1e-9)
# heat flow through the opaque south-west wall for a 10 K difference, by hand
chk("UA of the opaque south-west wall", float(env[0][6]) * float(by["U"][0][1]), (south - 8.1) / 1.87, 1e-9)

bad = 0
for ok, name, a, b in checks:
    bad += not ok
    print(f"{'PASS' if ok else 'FAIL'}  {name}" + ("" if ok else f":  {a} vs {b}"))
print(f"{len(checks)} checks, {bad} failed")
sys.exit(1 if bad else 0)
