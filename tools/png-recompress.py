#!/usr/bin/env python3
"""Re-deflate PNGs in place at zlib level 9. Standard library only.

    tools/png-recompress.py docs/cabinet/*.png

dump_frame writes stored-mode deflate (~2 MB a frame). make-cabinet-art.sh
shrinks its output with magick or optipng when either is installed; this
is the fallback for a machine with neither, so the art never has to be
committed at full size.

Lossless: the pixel data is decompressed and recompressed byte for byte.
Only IDAT is rewritten. Every other chunk is copied as it is.
"""

import struct
import sys
import zlib

SIGNATURE = b"\x89PNG\r\n\x1a\n"


def chunks(data):
    pos = len(SIGNATURE)
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos:pos + 4])
        kind = data[pos + 4:pos + 8]
        yield kind, data[pos + 8:pos + 8 + length]
        pos += 12 + length


def chunk(kind, body):
    crc = zlib.crc32(kind + body) & 0xFFFFFFFF
    return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", crc)


def recompress(path):
    data = open(path, "rb").read()
    if not data.startswith(SIGNATURE):
        sys.exit(f"{path}: not a PNG")
    parts = list(chunks(data))
    raw = zlib.decompress(b"".join(body for kind, body in parts if kind == b"IDAT"))

    out = [SIGNATURE]
    wrote_idat = False
    for kind, body in parts:
        if kind == b"IDAT":
            # Every IDAT collapses into one, written where the first was.
            if not wrote_idat:
                out.append(chunk(b"IDAT", zlib.compress(raw, 9)))
                wrote_idat = True
            continue
        out.append(chunk(kind, body))
    result = b"".join(out)

    # ⚠️ ONLY EVER SHRINK. An already-optimised file can come out larger.
    if len(result) < len(data):
        open(path, "wb").write(result)


if __name__ == "__main__":
    for p in sys.argv[1:]:
        recompress(p)
