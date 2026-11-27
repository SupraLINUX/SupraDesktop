import unittest
from trace import validate

GLOBALS = "\n".join(f"interface: '{name}', version: 1" for name in
                    ("wl_compositor", "wl_shm", "xdg_wm_base"))
GOOD = """xdg_wm_base#3.get_xdg_surface(new id xdg_surface#5, wl_surface#4)
xdg_surface#5.get_toplevel(new id xdg_toplevel#6)
xdg_surface#5.configure(17)
xdg_surface#5.ack_configure(17)
wl_surface#4.attach(wl_buffer#7, 0, 0)
wl_surface#4.frame(new id wl_callback#8)
wl_surface#4.commit()
wl_callback#8.done(100)
"""


class TraceTests(unittest.TestCase):
    def test_reference_and_legacy_object_syntax(self):
        for trace in (GOOD, GOOD.replace("#", "@")):
            self.assertTrue(validate(GLOBALS, trace)["configured_committed_frame"])

    def test_wrong_surface_serial_and_order_rejected(self):
        for trace in (GOOD.replace("ack_configure(17)", "ack_configure(18)"),
                      GOOD.replace("wl_surface#4.attach", "wl_surface#9.attach"),
                      GOOD.replace("wl_callback#8.done", "wl_callback#9.done"),
                      GOOD.replace("wl_surface#4.commit()", ""),
                      GOOD + "wl_display#1.error(wl_surface#4, 1, \"bad\")"):
            with self.subTest(trace=trace), self.assertRaises(ValueError):
                validate(GLOBALS, trace)
        with self.assertRaises(ValueError):
            validate(GLOBALS.replace("wl_shm", "other"), GOOD)
