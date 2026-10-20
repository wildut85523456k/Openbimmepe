import unittest
from worker import process


def run(expression, values):
    return process({"version": 1, "expression": expression, "values": values})["values"]


class WorkerTests(unittest.TestCase):
    def test_numeric_expressions(self):
        self.assertEqual(run("x + 0.3", [2.7, 3.7]), [3.0, 4.0])
        self.assertEqual(run("round(x, 2)", [1.234]), [1.23])
        self.assertEqual(run("sqrt(x)", [4.0]), [2.0])

    def test_invalid_results(self):
        for expression in ["1 / 0", "sqrt(-1)", "10 ** 10000", "True"]:
            with self.subTest(expression=expression), self.assertRaises(Exception):
                run(expression, [1.0])

    def test_restricted_syntax(self):
        for expression in ["__import__('os')", "x.real", "[x]", "open('file')", "(lambda: 1)()"]:
            with self.subTest(expression=expression), self.assertRaises(ValueError):
                run(expression, [1.0])
        for expression in ["True", "'text'", "x(1)", "__import__('os')"]:
            with self.assertRaises(ValueError):
                run(expression, [])

    def test_invalid_protocol_and_input(self):
        for values in [[True], [float("nan")], [float("inf")], ["1"]]:
            with self.assertRaises(ValueError):
                run("x", values)
        with self.assertRaises(ValueError):
            process({"version": 2, "expression": "x", "values": []})
        with self.assertRaises(ValueError):
            run("x", [1] * 10001)


if __name__ == "__main__":
    unittest.main()
