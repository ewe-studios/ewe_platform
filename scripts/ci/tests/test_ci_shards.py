"""Tests for the macOS/Windows shards and `run-many` (run: python3 -m unittest discover -s scripts/ci/tests)."""

import importlib.util
import sys
import unittest
from pathlib import Path
from unittest import mock

REPO = Path(__file__).resolve().parents[3]
CI = REPO / "scripts" / "ci" / "ci.py"


def load_ci():
    spec = importlib.util.spec_from_file_location("ci_shards_under_test", CI)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def item(name, weight=1, timeout=60, llama=False):
    return {"package": name, "weight": weight, "timeout": timeout, "llama": llama}


class Shards(unittest.TestCase):
    def setUp(self):
        self.ci = load_ci()
        self.items = [item(f"p{i:02}", weight=1 + i % 5) for i in range(23)]

    def names(self, shards):
        return [i["package"] for shard in shards for i in shard]

    def test_every_package_is_in_exactly_one_shard(self):
        shards = self.ci.split_shards(self.items, 4)
        self.assertEqual(sorted(self.names(shards)), sorted(i["package"] for i in self.items))

    def test_no_shard_is_empty_and_the_count_is_capped(self):
        self.assertEqual(len(self.ci.split_shards(self.items[:3], 4)), 3)
        self.assertTrue(all(self.ci.split_shards(self.items, 4)))
        self.assertEqual(self.ci.split_shards([], 4), [])

    def test_the_weights_are_balanced(self):
        loads = [sum(i["weight"] for i in shard) for shard in self.ci.split_shards(self.items, 4)]
        self.assertLessEqual(max(loads) - min(loads), 5)  # at most the heaviest package

    def test_the_split_is_deterministic(self):
        first = self.ci.split_shards(self.items, 4)
        second = self.ci.split_shards(list(reversed(self.items)), 4)
        self.assertEqual(first, second)

    def test_matrix_entries(self):
        items = [item("a", 2, 60, llama=True), item("b", 1, 90), item("c", 1, 250)]
        matrix = self.ci.shard_matrix(items, 2, 300)
        self.assertEqual([m["shard"] for m in matrix], ["1/2", "2/2"])
        self.assertEqual(sorted(" ".join(m["packages"] for m in matrix).split()), ["a", "b", "c"])
        by_pkg = {m["packages"]: m for m in matrix}
        self.assertTrue(by_pkg["a"]["llama"])
        self.assertEqual(by_pkg["b c"]["timeout"], 300)  # 340 capped at 300
        self.assertEqual(by_pkg["b c"]["weight"], 2)
        self.assertFalse(by_pkg["b c"]["llama"])


class RunMany(unittest.TestCase):
    def setUp(self):
        self.ci = load_ci()

    def fake_run(self, failing):
        def run(name, steps, profile, platform=None, report=None):
            if name in failing:
                report.append((name, [f"{name}: `cargo test` failed"], {f"{name}: `cargo test` failed": ["test x ... FAILED"]}))
                return 1
            return 0

        return run

    def test_it_continues_after_a_failure_and_exits_1(self):
        calls = []
        fake = self.fake_run({"b"})
        with mock.patch.object(self.ci, "run_package", side_effect=lambda *a, **k: (calls.append(a[0]), fake(*a, **k))[1]):
            self.assertEqual(self.ci.run_many(["a", "b", "c"], ["test"], "uat"), 1)
        self.assertEqual(calls, ["a", "b", "c"])

    def test_it_exits_0_when_all_pass(self):
        with mock.patch.object(self.ci, "run_package", side_effect=self.fake_run(set())):
            self.assertEqual(self.ci.run_many(["a", "b"], ["test"], "uat"), 0)

    def test_the_combined_summary_lists_each_failure(self):
        import contextlib, io

        out = io.StringIO()
        with mock.patch.object(self.ci, "run_package", side_effect=self.fake_run({"a", "c"})), contextlib.redirect_stdout(out):
            self.ci.run_many(["a", "b", "c"], ["test"], "uat")
        text = out.getvalue().split("==== summary")[1]
        self.assertIn("2 failed", text)
        self.assertIn("a: `cargo test` failed", text)
        self.assertIn("c: `cargo test` failed", text)
        self.assertIn("test x ... FAILED", text)
        self.assertNotIn("b:", text)

    def test_if_affected_skips_a_package(self):
        with mock.patch.object(self.ci, "run_package", side_effect=self.fake_run(set())) as run, \
             mock.patch.object(self.ci, "not_affected", side_effect=lambda n, b, h: n == "b"):
            self.ci.run_many(["a", "b"], ["test"], "uat", if_affected=True)
        self.assertEqual([c.args[0] for c in run.call_args_list], ["a"])


class PushBase(unittest.TestCase):
    def test_an_all_zero_base_selects_everything(self):
        ci = load_ci()
        packages = {"a": ci.Package("a", "a", False)}
        args = mock.Mock(packages=None, base="0" * 40, head=None)
        with mock.patch.dict("os.environ", {"CI_PACKAGES": ""}):
            names, why = ci.select(args, packages)
        self.assertEqual(names, {"a"})
        self.assertEqual(why, "full run")


if __name__ == "__main__":
    unittest.main()
