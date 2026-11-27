import unittest
from license_inventory import parse_tree


class ParseTreeTests(unittest.TestCase):
    def test_preserves_and_and_deduplicates(self):
        result = parse_tree("unicode-ident v1.0.27|(MIT OR Apache-2.0) AND Unicode-3.0\n"
                            "unicode-ident v1.0.27|(MIT OR Apache-2.0) AND Unicode-3.0 (*)\n"
                            "smallvil v0.1.0 (/tmp/smallvil)|\n")
        self.assertEqual(len(result), 2)
        self.assertEqual(result[("unicode-ident", "1.0.27")],
                         "(MIT OR Apache-2.0) AND Unicode-3.0")
        self.assertEqual(result[("smallvil", "0.1.0")], "")

    def test_conflict_rejected(self):
        with self.assertRaises(ValueError):
            parse_tree("demo v1.0.0|MIT\ndemo v1.0.0|Apache-2.0")

    def test_unknown_row_rejected(self):
        with self.assertRaises(ValueError):
            parse_tree("arbitrary output")
