import unittest
from multiclient import callbacks, report
from test_trace import GLOBALS, GOOD


class MultiClientTests(unittest.TestCase):
    def test_progress_after_exit(self):
        final = GOOD + "wl_callback#9.done(200)\n"
        result = report(GLOBALS, GOOD, final, 1)
        self.assertTrue(result["survivor_progress_after_peer_exit"])
        self.assertFalse(result["security_isolation_verified"])

    def test_no_progress_rejected(self):
        with self.assertRaises(ValueError):
            report(GLOBALS, GOOD, GOOD, 1)

    def test_protocol_error_rejected(self):
        with self.assertRaises(ValueError):
            callbacks(GOOD + "wl_display#1.error(wl_surface#4, 1, \"bad\")")
