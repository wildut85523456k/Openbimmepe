"""Version 1 numeric-expression worker. No eval, exec, imports or host API in expressions.

The interpreter evaluates the same expression once per scalar x. Quantities retain
their declared type; explicit unit conversions remain graph operations.
"""
import ast
import json
import math
import operator
import sys

LIMIT = 1_048_576
OPS = {ast.Add: operator.add, ast.Sub: operator.sub, ast.Mult: operator.mul,
       ast.Div: operator.truediv, ast.Mod: operator.mod, ast.Pow: math.pow}
FUNCTIONS = {"abs": abs, "min": min, "max": max, "round": round,
             "sqrt": math.sqrt, "sin": math.sin, "cos": math.cos}


def number(value):
    if isinstance(value, bool) or not isinstance(value, (float, int)):
        raise ValueError("Only finite numeric values are supported")
    value = float(value)
    if not math.isfinite(value):
        raise ValueError("Non-finite result")
    return value


def evaluate(node, x):
    if isinstance(node, ast.Constant):
        return number(node.value)
    if isinstance(node, ast.Name) and node.id == "x":
        return x
    if isinstance(node, ast.UnaryOp) and isinstance(node.op, (ast.UAdd, ast.USub)):
        result = evaluate(node.operand, x)
        return result if isinstance(node.op, ast.UAdd) else -result
    if isinstance(node, ast.BinOp) and type(node.op) in OPS:
        return number(OPS[type(node.op)](evaluate(node.left, x), evaluate(node.right, x)))
    if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id in FUNCTIONS:
        if node.keywords or not 1 <= len(node.args) <= 8:
            raise ValueError("Invalid function arguments")
        args = [evaluate(arg, x) for arg in node.args]
        if node.func.id == "round" and len(args) == 2:
            if not args[1].is_integer() or abs(args[1]) > 308:
                raise ValueError("Invalid rounding precision")
            args[1] = int(args[1])
        return number(FUNCTIONS[node.func.id](*args))
    raise ValueError("Unsupported expression; use numeric x, arithmetic and approved functions")


def process(request):
    if not isinstance(request, dict) or set(request) != {"version", "expression", "values"}:
        raise ValueError("Invalid request fields")
    if type(request["version"]) is not int or request["version"] != 1:
        raise ValueError("Unsupported protocol version")
    expression, values = request["expression"], request["values"]
    if not isinstance(expression, str) or len(expression.encode("utf-8")) > 4096:
        raise ValueError("Expression limit exceeded")
    if not isinstance(values, list) or len(values) > 10_000:
        raise ValueError("Value limit exceeded")
    tree = ast.parse(expression, mode="eval")
    if sum(1 for _ in ast.walk(tree)) > 256:
        raise ValueError("Expression complexity limit exceeded")
    # Validate even for empty input lists.
    allowed = (ast.Expression, ast.Constant, ast.Name, ast.Load, ast.UnaryOp,
               ast.UAdd, ast.USub, ast.BinOp, ast.Call, *OPS.keys())
    for node in ast.walk(tree):
        if not isinstance(node, allowed):
            raise ValueError("Unsupported expression syntax")
        if isinstance(node, ast.Name) and node.id not in {"x", *FUNCTIONS}:
            raise ValueError("Unknown expression name")
        if isinstance(node, ast.Constant):
            number(node.value)
        if isinstance(node, ast.Call):
            if not isinstance(node.func, ast.Name) or node.func.id not in FUNCTIONS:
                raise ValueError("Unsupported function")
            if node.keywords or not 1 <= len(node.args) <= 8:
                raise ValueError("Invalid function arguments")
    return {"version": 1, "values": [number(evaluate(tree.body, number(x))) for x in values]}


def main():
    try:
        raw = sys.stdin.buffer.read(LIMIT + 1)
        if len(raw) > LIMIT:
            raise ValueError("Request limit exceeded")
        response = process(json.loads(raw))
    except Exception as exc:
        response = {"version": 1, "error": str(exc)[:1024]}
    sys.stdout.write(json.dumps(response, allow_nan=False))


if __name__ == "__main__":
    main()
