"""Independent check of the family categories and MEP connectors (S2f).

Reads the output of `cargo run --example fan_scene` (stdin or file argument) and
 1. checks every IFC class / enum name the Rust code emits exists in the IFC4 schema,
 2. recomputes the placed connector positions and directions with its own maths,
 3. writes a real IFC4 file (equipment + IfcDistributionPort nested in it), reads the ports back
    through IfcOpenShell's placement code and compares them with the Rust figures,
 4. recomputes the connection (mate) decisions with its own implementation,
 5. runs negative controls (a perturbed value must be reported as a mismatch).
"""
import math, sys, tempfile, os
import numpy as np
import ifcopenshell, ifcopenshell.api, ifcopenshell.guid
import ifcopenshell.ifcopenshell_wrapper as w
import ifcopenshell.util.placement as up

text = open(sys.argv[1]).read() if len(sys.argv) > 1 else sys.stdin.read()
rows = [l.split() for l in text.splitlines() if l.split() and l.split()[0] in ("CLASS", "SYS", "FLOW", "PORT", "POSE", "LOCAL", "WORLD", "MATE", "SELF")]
schema = w.schema_by_name("IFC4")
checks = fails = 0
def ok(cond, msg):
    global checks, fails
    checks += 1
    if not cond:
        fails += 1
        print("FAIL", msg)

# 1. names exist in the schema
sysenum = schema.declaration_by_name("IfcDistributionSystemEnum").enumeration_items()
flowenum = schema.declaration_by_name("IfcFlowDirectionEnum").enumeration_items()
portenum = schema.declaration_by_name("IfcDistributionPortTypeEnum").enumeration_items()
for r in rows:
    if r[0] == "CLASS":
        d = schema.declaration_by_name(r[2])
        ok(d is not None, f"class {r[2]}")
        ok(not d.is_abstract(), f"{r[2]} must not be abstract")
    elif r[0] == "SYS": ok(r[2] in sysenum, f"system {r[2]}")
    elif r[0] == "FLOW": ok(r[2] in flowenum, f"flow {r[2]}")
    elif r[0] == "PORT": ok(r[2] in portenum, f"port type {r[2]}")

poses = {r[1]: tuple(map(float, r[2:6])) for r in rows if r[0] == "POSE"}
def parse(r):
    name, dom, sysn, fd = r[2], r[3], r[4], r[5]
    pos = tuple(map(float, r[6:9])); d = tuple(map(float, r[9:12]))
    return dict(name=name, dom=dom, sys=sysn, fd=fd, pos=pos, dir=d, size=r[12:-2], flow=float(r[-2]), pd=float(r[-1]))
local = {}; world = {}
for r in rows:
    if r[0] in ("LOCAL", "WORLD"):
        (local if r[0] == "LOCAL" else world)[(r[1], r[2])] = parse(r)
    # r layout: TAG k name ... ; parse() expects index 2.. => shift handled by slicing r[1:]
def rot(v, a):
    s, c = math.sin(math.radians(a)), math.cos(math.radians(a))
    return (c*v[0]-s*v[1], s*v[0]+c*v[1], v[2])

# 2. own maths for placed connectors
for (k, n), lc in local.items():
    x, y, z, a = poses[k]
    p = rot(lc["pos"], a); d = rot(lc["dir"], a)
    wc = world[(k, n)]
    ok(np.allclose([p[0]+x, p[1]+y, p[2]+z], wc["pos"], atol=1e-7), f"world pos {k} {n}")
    ok(np.allclose(d, wc["dir"], atol=1e-7), f"world dir {k} {n}")

# 3. real IFC file with ports
def build(pose_key, perturb=0.0):
    f = ifcopenshell.file(schema="IFC4")
    run = ifcopenshell.api.run
    run("root.create_entity", f, ifc_class="IfcProject", name="p")
    eq = run("root.create_entity", f, ifc_class="IfcUnitaryEquipment", name="FCU")
    x, y, z, a = poses[pose_key]
    m = np.eye(4)
    c, s = math.cos(math.radians(a)), math.sin(math.radians(a))
    m[:2, :2] = [[c, -s], [s, c]]; m[:3, 3] = [x, y, z]
    run("geometry.edit_object_placement", f, product=eq, matrix=m)
    ports = []
    for (k, n), lc in local.items():
        if k != pose_key: continue
        port = f.create_entity("IfcDistributionPort", GlobalId=ifcopenshell.guid.new(), Name=n,
                               FlowDirection={"In": "SINK", "Out": "SOURCE", "Both": "SOURCEANDSINK"}[lc["fd"]],
                               PredefinedType={"Duct": "DUCT", "Pipe": "PIPE", "CableTray": "CABLECARRIER", "Conduit": "CABLECARRIER", "Electrical": "CABLE"}[lc["dom"]])
        dz = np.array(lc["dir"]); ref = np.array([1.0, 0, 0]) if abs(dz[0]) < 0.9 else np.array([0, 1.0, 0])
        ref = ref - dz * ref.dot(dz); ref /= np.linalg.norm(ref)
        pos = np.array(lc["pos"]); pos[0] += perturb
        axes = f.create_entity("IfcAxis2Placement3D", Location=f.create_entity("IfcCartesianPoint", Coordinates=tuple(pos)),
                               Axis=f.create_entity("IfcDirection", DirectionRatios=tuple(dz)), RefDirection=f.create_entity("IfcDirection", DirectionRatios=tuple(ref)))
        port.ObjectPlacement = f.create_entity("IfcLocalPlacement", PlacementRelTo=eq.ObjectPlacement, RelativePlacement=axes)
        f.create_entity("IfcRelNests", GlobalId=ifcopenshell.guid.new(), RelatingObject=eq, RelatedObjects=[port])
        ports.append(port)
    path = os.path.join(tempfile.mkdtemp(), "fcu.ifc"); f.write(path)
    return path

def compare(path, pose_key):
    g = ifcopenshell.open(path)
    bad = 0; n_ports = 0
    for port in g.by_type("IfcDistributionPort"):
        n_ports += 1
        mtx = up.get_local_placement(port.ObjectPlacement)
        wc = world[(pose_key, port.Name)]
        if not np.allclose(mtx[:3, 3], wc["pos"], atol=1e-7): bad += 1
        if not np.allclose(mtx[:3, 2], wc["dir"], atol=1e-7): bad += 1
        want = {"In": "SINK", "Out": "SOURCE", "Both": "SOURCEANDSINK"}[wc["fd"]]
        if port.FlowDirection != want: bad += 1
        nests = [r for r in g.by_type("IfcRelNests") if port in r.RelatedObjects]
        if len(nests) != 1 or nests[0].RelatingObject.is_a() != "IfcUnitaryEquipment": bad += 1
    return bad, n_ports

for k in poses:
    bad, n = compare(build(k), k)
    ok(n == 7 and bad == 0, f"ifc ports pose {k}: {n} ports, {bad} mismatches")
    nb, _ = compare(build(k, perturb=0.01), k)   # negative control
    ok(nb > 0, f"negative control pose {k} must report mismatches")

# 4. own mate implementation
def mate(a, b, tol=0.001):
    if a["dom"] != b["dom"]: return "Domain"
    if not (a["sys"] == b["sys"] or a["sys"] == "Other" or b["sys"] == "Other"): return "System"
    if a["fd"] == b["fd"] and a["fd"] in ("In", "Out"): return "FlowDirection"
    if np.linalg.norm(np.array(a["pos"]) - np.array(b["pos"])) > tol: return "Position"
    if np.dot(a["dir"], b["dir"]) > -1 + 1e-9: return "Facing"
    if a["size"][0] != b["size"][0]: return "ShapeKind"
    if a["size"][0] in ("round", "rect"):
        if any(abs(float(x) - float(y)) > 1e-9 for x, y in zip(a["size"][1:], b["size"][1:])): return "Size"
    elif a["size"][0] == "signal":
        pass
    else:
        if abs(float(a["size"][1]) - float(b["size"][1])) > 1e-9: return "Voltage"
        if a["size"][2] != b["size"][2]: return "Poles"
    return "Ok"
w0 = {n: c for (k, n), c in world.items() if k == "0"}
w1 = {n: c for (k, n), c in world.items() if k == "1"}
cnt = 0
for r in rows:
    if r[0] == "MATE":
        cnt += 1
        res = mate(w0[r[1]], w1[r[2]])
        want = "Ok" if r[3] == "Ok(())" else r[3][4:-1]
        ok(res == want, f"mate {r[1]}/{r[2]}: rust {r[3]} python {res}")
    if r[0] == "SELF":
        a = w0[r[1]]
        f = dict(a); f["dir"] = tuple(-v for v in a["dir"]); f["fd"] = {"In": "Out", "Out": "In", "Both": "Both"}[a["fd"]]
        ok(mate(a, f) == "Ok" and r[2] == "Ok(())", f"self {r[1]}")
ok(cnt == 49, f"expected 49 mate lines, got {cnt}")
# the agreed connector set (15 Oct 2026), written out independently of the Rust family:
# 2 chilled water, 2 air ductwork, 1 drainage, 1 power, 1 BMS
want = {"chw_in": ("Pipe", "ChilledWaterSupply"), "chw_out": ("Pipe", "ChilledWaterReturn"),
        "supply": ("Duct", "SupplyAir"), "return": ("Duct", "ReturnAir"),
        "condensate": ("Pipe", "Condensate"), "power": ("Electrical", "Power"), "bms": ("Electrical", "Controls")}
got = {n: (c["dom"], c["sys"]) for (k, n), c in local.items() if k == "0"}
ok(got == want, f"fan coil connector set: {got}")
bms = local.get(("0", "bms"), {})
ok(bms.get("size") == ["signal"] and bms.get("fd") == "Both", "bms is a two-way signal link")
sysmap = {r[1]: r[2] for r in rows if r[0] == "SYS"}
ok(sysmap.get("Controls") == "CONTROL", "Controls maps to IfcDistributionSystemEnum CONTROL")
# negative control for the mate logic: a connector must not join the unmoved original of a different pose
ok(mate(w0["supply"], w1["supply"]) != "Ok", "negative control mate")
print(f"{checks} checks, {fails} failures")
sys.exit(1 if fails else 0)
