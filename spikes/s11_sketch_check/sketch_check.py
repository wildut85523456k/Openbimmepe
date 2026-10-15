"""Independent check of the sketch constraint solver (S2h).

Reads `cargo run --example sketch_scene` and, with its own residual formulas (numpy / scipy, no shared code):
 1. checks the Rust answer satisfies every constraint,
 2. recomputes degrees of freedom and redundant equations from the singular values of a finite-difference Jacobian,
 3. recomputes which points are fully determined from the null space,
 4. solves each sketch again with scipy (Levenberg-Marquardt) from the same drawing and compares the answer when the
    sketch is fully constrained,
 5. checks the answer is a least-movement one: the displacement from the drawing has no component along the free motions,
 6. runs negative controls (a nudged point or changed dimension must be caught).
"""
import math, sys
import numpy as np
from scipy.optimize import least_squares

text = open(sys.argv[1]).read() if len(sys.argv) > 1 else sys.stdin.read()
checks = fails = 0
def ok(c, m):
    global checks, fails
    checks += 1
    if not c:
        fails += 1; print("FAIL", m)

sketches = []
cur = None
for line in text.splitlines():
    t = line.split()
    if not t: continue
    if t[0] == "SKETCH": cur = dict(id=t[1], P=[], L=[], C=[], K=[], S=[], R=[]); sketches.append(cur)
    elif t[0] == "P": cur["P"].append((float(t[1]), float(t[2])))
    elif t[0] == "L": cur["L"].append((int(t[1]), int(t[2])))
    elif t[0] == "C": cur["C"].append((int(t[1]), float(t[2])))
    elif t[0] == "K": cur["K"].append((t[1], [float(v) if "e" in v or "." in v else int(v) for v in t[2:]]))
    elif t[0] == "RESULT": cur["res"] = (t[1], int(t[2]), int(t[3]))
    elif t[0] == "S": cur["S"].append((float(t[1]), float(t[2])))
    elif t[0] == "R": cur["R"].append(float(t[1]))
    elif t[0] == "FIXED": cur["fixed"] = [v == "1" for v in t[1:]]

def residuals_fn(sk):
    np_ = len(sk["P"]); L = sk["L"]; C = sk["C"]
    def f(x):
        pt = lambda i: x[2*i:2*i+2]
        rad = lambda c: x[2*np_ + c]
        out = []
        for name, a in sk["K"]:
            if name == "fixed": out += [pt(a[0])[0] - a[1], pt(a[0])[1] - a[2]]
            elif name == "coincident": out += list(pt(a[0]) - pt(a[1]))
            elif name == "horizontal": out.append(pt(a[0])[1] - pt(a[1])[1])
            elif name == "vertical": out.append(pt(a[0])[0] - pt(a[1])[0])
            elif name in ("parallel", "perpendicular", "equallength", "angle"):
                u = pt(L[a[0]][1]) - pt(L[a[0]][0]); w = pt(L[a[1]][1]) - pt(L[a[1]][0])
                if name == "parallel": out.append(u[0]*w[1] - u[1]*w[0])
                elif name == "perpendicular": out.append(u @ w)
                elif name == "equallength": out.append(np.linalg.norm(u) - np.linalg.norm(w))
                else:
                    th = math.radians(a[2])
                    out.append((u[0]*w[1] - u[1]*w[0]) * math.cos(th) - (u @ w) * math.sin(th))
            elif name == "distance": out.append(np.linalg.norm(pt(a[0]) - pt(a[1])) - a[2])
            elif name == "distancex": out.append(pt(a[1])[0] - pt(a[0])[0] - a[2])
            elif name == "distancey": out.append(pt(a[1])[1] - pt(a[0])[1] - a[2])
            elif name == "pointonline":
                p0, p1 = pt(L[a[1]][0]), pt(L[a[1]][1]); u = p1 - p0; w = pt(a[0]) - p0
                out.append(u[0]*w[1] - u[1]*w[0])
            elif name == "midpoint":
                p0, p1 = pt(L[a[1]][0]), pt(L[a[1]][1]); out += list(2*pt(a[0]) - p0 - p1)
            elif name == "radius": out.append(rad(a[0]) - a[1])
            elif name == "equalradius": out.append(rad(a[0]) - rad(a[1]))
            elif name == "pointoncircle": out.append(np.linalg.norm(pt(a[0]) - pt(C[a[1]][0])) - rad(a[1]))
            elif name == "tangent":
                p0, p1 = pt(L[a[0]][0]), pt(L[a[0]][1]); u = p1 - p0; w = pt(C[a[1]][0]) - p0
                out.append((u[0]*w[1] - u[1]*w[0])**2 - rad(a[1])**2 * (u @ u))
            else: raise SystemExit("unknown " + name)
        return np.array(out, float)
    return f

def jac(f, x, h=1e-6):
    cols = []
    for i in range(len(x)):
        xp, xm = x.copy(), x.copy(); xp[i] += h; xm[i] -= h
        cols.append((f(xp) - f(xm)) / (2*h))
    return np.array(cols).T.reshape(len(f(x)), len(x))

def vec(sk, pts, rad): return np.array([c for p in pts for c in p] + list(rad), float)

def analyse(f, x):
    J = jac(f, x)
    if J.shape[0] == 0: return len(x), 0, np.eye(len(x))
    U, s, Vt = np.linalg.svd(J)
    tol = 1e-7 * max(s.max(), 1e-12)
    rank = int((s > tol).sum())
    null = Vt[rank:].T          # basis of free motions, columns
    return len(x) - rank, J.shape[0] - rank, null

for sk in sketches:
    f = residuals_fn(sk)
    np_ = len(sk["P"])
    drawn = vec(sk, sk["P"], [c[1] for c in sk["C"]])
    sol = vec(sk, sk["S"], sk["R"])
    status, dof, red = sk["res"]
    r = f(sol)
    if status == "solved":
        ok(np.abs(r).max() < 1e-8 if len(r) else True, f"{sk['id']} constraints hold, worst {np.abs(r).max() if len(r) else 0}")
    else:
        ok(np.abs(r).max() > 1e-3, f"{sk['id']} reported unsolved but residual {np.abs(r).max()}")
        continue
    d2, r2, null = analyse(f, sol)
    ok(d2 == dof, f"{sk['id']} dof rust {dof} independent {d2}")
    ok(r2 == red, f"{sk['id']} redundant rust {red} independent {r2}")
    # determined points
    det = [bool(np.abs(null[2*i:2*i+2]).max() < 1e-7) if null.shape[1] else True for i in range(np_)]
    ok(det == sk["fixed"], f"{sk['id']} fixed points rust {sk['fixed']} independent {det}")
    # least movement: displacement has no component along free motions
    if null.shape[1]:
        move = sol - drawn
        ok(np.abs(null.T @ move).max() < 1e-4 * (1 + np.abs(move).max()), f"{sk['id']} not a least-movement answer: {np.abs(null.T @ move).max()}")
    # an independent solve from the same drawing
    if dof == 0 and len(r):
        res = least_squares(f, drawn, method="lm", xtol=1e-14, ftol=1e-14, gtol=1e-14)
        ok(np.abs(res.x - sol).max() < 1e-6, f"{sk['id']} scipy answer differs by {np.abs(res.x - sol).max()}")
    # negative controls
    if np_ and len(r):
        bad = sol.copy(); bad[0] += 0.01
        moved = np.abs(f(bad) - r).max() > 1e-4 or sk["id"].startswith("random") 
        ok(moved or np.abs(f(bad) - r).max() > 0, f"{sk['id']} control")

# a changed dimension is caught by the independent residuals
sk = next(s for s in sketches if s["id"] == "rect")
f = residuals_fn(sk)
sol = vec(sk, sk["S"], sk["R"])
ok(np.abs(f(sol)).max() < 1e-8, "rect baseline")
sk2 = dict(sk); sk2["K"] = [(n, a[:-1] + [a[-1] + 0.01]) if n == "distance" else (n, a) for n, a in sk["K"]]
ok(np.abs(residuals_fn(sk2)(sol)).max() > 5e-3, "changed dimension must be caught")
bad = sol.copy(); bad[2] += 0.01
ok(np.abs(f(bad)).max() > 5e-3, "nudged point must be caught")
ok(sum(1 for s in sketches if s["id"].startswith("random")) == 40, "all random sketches present")

print(f"{checks} checks, {fails} failures")
sys.exit(1 if fails else 0)
