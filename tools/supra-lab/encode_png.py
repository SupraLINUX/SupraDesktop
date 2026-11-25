"""Export CI fixture PPMs using only Python stdlib."""
from pathlib import Path
import re
import struct
import zlib

def chunk(kind, data):
    return struct.pack(">I",len(data))+kind+data+struct.pack(">I",zlib.crc32(kind+data)&0xffffffff)

def convert(path):
    data=path.read_bytes()
    header=re.match(rb"P6\n(\d+) (\d+)\n255\n",data)
    if not header:
        raise ValueError("Unsupported PPM")
    w,h=map(int,header.groups())
    pixels=data[header.end():]
    if len(pixels)!=w*h*3:
        raise ValueError("Invalid payload")
    raw=b"".join(b"\0"+pixels[y*w*3:(y+1)*w*3] for y in range(h))
    path.with_suffix(".png").write_bytes(b"\x89PNG\r\n\x1a\n"+chunk(b"IHDR",struct.pack(">IIBBBBB",w,h,8,2,0,0,0))+chunk(b"IDAT",zlib.compress(raw))+chunk(b"IEND",b""))

if __name__=="__main__":
    for path in sorted(Path("evidence").glob("*.ppm")):
        convert(path)
