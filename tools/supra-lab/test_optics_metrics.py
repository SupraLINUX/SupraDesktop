import unittest
from optics_metrics import compare, validate


class OpticsMetricsTests(unittest.TestCase):
    def test_local_change(self):
        # Two 2x2 panels. One changed RGB pixel inside ROI.
        payload = bytearray(4 * 2 * 3)
        payload[6:9] = bytes([10, 20, 30])
        result = compare(4, 2, payload, (0, 0, 1, 1))
        self.assertEqual(result["inside_roi"]["changed_pixels"], 1)
        self.assertEqual(result["inside_roi"]["mean_absolute_channel_error"], 20)
        self.assertEqual(result["inside_roi"]["p95_absolute_channel_error"], 30)
        self.assertEqual(result["outside_roi"]["changed_pixels"], 0)
        validate(result)

    def test_leak_or_identity(self):
        payload = bytearray(4 * 2 * 3)
        with self.assertRaises(ValueError):
            validate(compare(4, 2, payload, (0, 0, 1, 1)))
        payload[9] = 1
        with self.assertRaises(ValueError):
            validate(compare(4, 2, payload, (0, 0, 1, 1)))

    def test_bad_dimensions(self):
        for w, h, roi in ((3, 2, (0, 0, 1, 1)), (4, 2, (0, 0, 3, 1))):
            with self.assertRaises(ValueError):
                compare(w, h, bytes(w*h*3), roi)
