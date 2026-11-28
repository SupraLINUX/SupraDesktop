import unittest
from pathlib import Path
from evidence import verify

TRACE = Path(__file__).with_name("reused-callback.trace").read_text()
RENDERER = "SUPRA_NESTED_FRAME_1_COMPLETE\nSUPRA_NESTED_FRAME_2_COMPLETE\nSUPRA_NESTED_BUFFERS_RELEASED\n"


class EvidenceTests(unittest.TestCase):
    def test_reused_callback_id_is_not_third_frame(self):
        verify(TRACE, RENDERER)

    def test_missing_or_premature_frame_rejected(self):
        for bad in (
            TRACE.replace("[22] wl_callback@12.done(187935)\n", ""),
            TRACE.replace("[19] -> wl_surface@3.commit()", "[19] wl_callback@12.done(187935)"),
            TRACE.replace("[9] -> xdg_surface@7.ack_configure(1)", "[9] -> xdg_surface@7.ack_configure(2)"),
            TRACE.replace("[25] wl_buffer@9.release()\n", ""),
            TRACE.replace("[25] wl_buffer@9.release()", "[25] wl_buffer@10.release()"),
        ):
            with self.subTest(trace=bad), self.assertRaises(ValueError):
                verify(bad, RENDERER)
        with self.assertRaises(ValueError):
            verify(TRACE, RENDERER.replace("FRAME_2", "FRAME_1"))
