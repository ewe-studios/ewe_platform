from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "ui"))
from check_behaviours import check


class BehaviourChecks(unittest.TestCase):
    def test_bindings_and_wire_forms(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "Sources/EweUI"
            source.mkdir(parents=True)
            (source / "DialogBinding.swift").write_text("struct DialogBinding {}")
            rows = {"dialog":{"shared":"DIALOG","ios":"DialogBinding"}, "hover":{"custom":"hover"}}
            matrix = {"behaviours":rows,"components":{"sheet":{"behaviours":["dialog"],"native":True}}}
            self.assertEqual(check(matrix,"ios",root),[])
            self.assertIn("has no android form", check(matrix,"android",root)[0])
            rows["dialog"]["ios"]="Missing"
            self.assertIn("binding `Missing` not found",check(matrix,"ios",root)[0])
            rows["hover"]={}
            matrix["components"]["sheet"]["behaviours"].append("new")
            errors=check(matrix,"ios",root)
            self.assertIn("hover: expected one wire form",errors)
            self.assertIn("sheet: unknown behaviour `new`",errors)


if __name__ == "__main__":
    unittest.main()
