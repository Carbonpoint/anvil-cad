#!/usr/bin/env python3
"""Convert a binary PPM (P6) to PNG with only the standard library."""
import struct, sys, zlib

def ppm2png(src, dst):
    data = open(src, 'rb').read()
    parts = data.split(maxsplit=4)
    w, h = int(parts[1]), int(parts[2])
    pixels = parts[4][: w * h * 3]
    raw = b''.join(b'\x00' + pixels[y * w * 3:(y + 1) * w * 3] for y in range(h))
    def chunk(tag, body):
        c = tag + body
        return struct.pack('>I', len(body)) + c + struct.pack('>I', zlib.crc32(c) & 0xffffffff)
    png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0))
    png += chunk(b'IDAT', zlib.compress(raw, 9)) + chunk(b'IEND', b'')
    open(dst, 'wb').write(png)

if __name__ == '__main__':
    ppm2png(sys.argv[1], sys.argv[2])
