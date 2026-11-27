import unittest
from native_inventory import shared_paths


class RuntimeInventoryTests(unittest.TestCase):
    def test_filters_mappings_and_deduplicates(self):
        rows = [
            "100-200 r-xp 0 00:01 1 /usr/lib/x86_64-linux-gnu/libfoo.so.1",
            "200-300 r--p 0 00:01 1 /usr/lib/x86_64-linux-gnu/libfoo.so.1",
            "300-400 r-xp 0 00:01 2 /lib/dri/swrast_dri.so",
            "400-500 rw-p 0 00:00 0 [heap]",
            "500-600 r--p 0 00:01 3 /usr/share/data",
            "600-700 r-xp 0 00:01 4 /tmp/private.so",
        ]
        self.assertEqual(shared_paths("\n".join(rows)),
                         ["/lib/dri/swrast_dri.so", "/usr/lib/x86_64-linux-gnu/libfoo.so.1"])
