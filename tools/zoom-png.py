"""Nearest-neighbour zoom of a PNG region, stdlib only.

Exists because a 24x30 sprite does not survive the trip to my eyes at
native size (LESSONS L063). Blowing it up 8x with NO interpolation means
a 2px gap arrives as 16px and cannot be smeared away.
"""
import sys, zlib, struct

def read_png(path):
    d = open(path,'rb').read()
    assert d[:8] == b'\x89PNG\r\n\x1a\n'
    i, idat, w, h = 8, b'', 0, 0
    while i < len(d):
        ln = struct.unpack('>I', d[i:i+4])[0]; kind = d[i+4:i+8]; body = d[i+8:i+8+ln]
        if kind == b'IHDR':
            w, h, bd, ct = struct.unpack('>IIBB', body[:10])
            assert bd == 8 and ct == 2, (bd, ct)
        elif kind == b'IDAT': idat += body
        i += 12 + ln
    raw = zlib.decompress(idat)
    px, stride, prev = [], w*3, bytearray(w*3)
    pos = 0
    for _ in range(h):
        f = raw[pos]; pos += 1
        line = bytearray(raw[pos:pos+stride]); pos += stride
        if f == 1:
            for x in range(3, stride): line[x] = (line[x] + line[x-3]) & 255
        elif f == 2:
            for x in range(stride): line[x] = (line[x] + prev[x]) & 255
        elif f == 3:
            for x in range(stride):
                a = line[x-3] if x >= 3 else 0
                line[x] = (line[x] + ((a + prev[x]) >> 1)) & 255
        elif f == 4:
            for x in range(stride):
                a = line[x-3] if x >= 3 else 0
                b = prev[x]; c = prev[x-3] if x >= 3 else 0
                p = a + b - c
                pa, pb, pc = abs(p-a), abs(p-b), abs(p-c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[x] = (line[x] + pr) & 255
        px.append(bytes(line)); prev = line
    return w, h, px

def write_png(path, w, h, rows):
    raw = b''.join(b'\x00' + r for r in rows)
    def chunk(k, b):
        return struct.pack('>I', len(b)) + k + b + struct.pack('>I', zlib.crc32(k+b) & 0xffffffff)
    out = b'\x89PNG\r\n\x1a\n'
    out += chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0))
    out += chunk(b'IDAT', zlib.compress(raw, 9))
    out += chunk(b'IEND', b'')
    open(path,'wb').write(out)

src, dst, x0, y0, cw, ch, z = sys.argv[1], sys.argv[2], *map(int, sys.argv[3:8])
w, h, px = read_png(src)
rows = []
for y in range(y0, y0+ch):
    line = bytearray()
    for x in range(x0, x0+cw):
        p = px[y][x*3:x*3+3] if 0 <= y < h and 0 <= x < w else b'\x00\x00\x00'
        line += p * z
    for _ in range(z): rows.append(bytes(line))
write_png(dst, cw*z, ch*z, rows)
print(f"{dst}: {cw}x{ch} region at {z}x -> {cw*z}x{ch*z}")
