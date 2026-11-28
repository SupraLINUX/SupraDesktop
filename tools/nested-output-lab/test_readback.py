import unittest
from PIL import Image
from readback import locate, compare


def frame(seed):
    im = Image.new("RGB", (160,96))
    im.putdata([((x*7+y+seed)%256, (y*5+seed)%256, (x+y*3)%256)
                for y in range(96) for x in range(160)])
    return im


def capture(source, x=23, y=41):
    host = Image.new("RGB", (800,600), (13,17,19))
    host.paste(source, (x,y))
    return host


class ReadbackTests(unittest.TestCase):
    def test_exact_unscaled_changed_frames(self):
        a,b = frame(1),frame(2)
        result = compare((capture(a),a),(capture(b),b))
        self.assertEqual([r["different_pixels"] for r in result], [0,0])
        self.assertEqual((result[0]["x"],result[0]["y"]), (23,41))

    def test_mismatch_ambiguity_and_movement(self):
        a,b = frame(1),frame(2)
        bad = capture(a)
        bad.putpixel((23,41),(0,0,0))
        with self.assertRaises(ValueError):
            locate(bad,a)
        dup = capture(a)
        dup.paste(a,(240,41))
        with self.assertRaises(ValueError):
            locate(dup,a)
        with self.assertRaises(ValueError):
            compare((capture(a),a),(capture(b,24,41),b))
        with self.assertRaises(ValueError):
            compare((capture(a),a),(capture(a),a))
