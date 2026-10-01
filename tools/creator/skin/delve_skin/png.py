"""A PNG writer whose bytes are a function of the pixels and nothing else.

A composed skin is committed to a campaign and baked into the delve's resource
pack as those exact bytes (ADR-0006), so the encoder is part of the toolchain and
is written here rather than borrowed. Every byte this module emits is fixed by
the PNG specification (ISO/IEC 15948, W3C PNG 3rd ed.), RFC 1950 (zlib) and
RFC 1951 (deflate) given the pixels:

- one IHDR (8-bit RGBA, no interlace), one IDAT, one IEND, no ancillary chunk;
- filter type 0 (None) on every scanline;
- the zlib stream is header ``78 01`` followed by STORED deflate blocks (BTYPE
  00) of at most 65535 bytes each, the last one flagged BFINAL, then the Adler-32
  of the scanlines.

A stored block carries its bytes verbatim, so there is no compressor and no
choice for a library, a build or a CPU to make differently. CRC-32 and Adler-32
are checksums, not encoders: every correct implementation of either returns the
same value. Nothing here imports `zlib`, so no linked deflate is reachable.

The price is size: a 64x64 RGBA skin is 16516 bytes, against roughly 1.9-2.4 kB
deflated.
"""

from __future__ import annotations

import binascii
import struct

SIGNATURE = b"\x89PNG\r\n\x1a\n"

#: CMF 0x78 (deflate, 32 KiB window) and FLG 0x01 (FLEVEL 0, no dictionary),
#: whose FCHECK makes ``0x7801 % 31 == 0`` (RFC 1950 §2.2).
ZLIB_HEADER = b"\x78\x01"

#: The largest payload a stored deflate block can carry: LEN is 16 bits
#: (RFC 1951 §3.2.4).
STORED_BLOCK_MAX = 0xFFFF

_ADLER_MOD = 65521


def adler32(data: bytes) -> int:
    """Adler-32 (RFC 1950 §8.2)."""
    a, b = 1, 0
    for i in range(0, len(data), 5552):
        for byte in data[i : i + 5552]:
            a += byte
            b += a
        a %= _ADLER_MOD
        b %= _ADLER_MOD
    return (b << 16) | a


def _chunk(kind: bytes, data: bytes) -> bytes:
    crc = binascii.crc32(kind + data) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", crc)


def zlib_stored(raw: bytes) -> bytes:
    """``raw`` as a zlib stream of stored deflate blocks (no compression)."""
    out = bytearray(ZLIB_HEADER)
    starts = range(0, len(raw), STORED_BLOCK_MAX) if raw else [0]
    last = starts[-1]
    for start in starts:
        block = raw[start : start + STORED_BLOCK_MAX]
        # BFINAL in bit 0, BTYPE 00 in bits 1-2; a stored block's header is then
        # padded to the byte boundary, so the whole header is this one byte.
        out.append(1 if start == last else 0)
        out += struct.pack("<HH", len(block), len(block) ^ 0xFFFF)
        out += block
    out += struct.pack(">I", adler32(raw))
    return bytes(out)


def encode_rgba(width: int, height: int, rgba: bytes) -> bytes:
    """An 8-bit RGBA image (row-major, 4 bytes a pixel) as PNG bytes."""
    stride = width * 4
    if width <= 0 or height <= 0 or len(rgba) != stride * height:
        raise ValueError(
            f"{len(rgba)} bytes is not a {width}x{height} RGBA image "
            f"({stride * height} bytes)"
        )
    raw = b"".join(
        b"\x00" + rgba[y * stride : (y + 1) * stride] for y in range(height)
    )
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return (
        SIGNATURE
        + _chunk(b"IHDR", ihdr)
        + _chunk(b"IDAT", zlib_stored(raw))
        + _chunk(b"IEND", b"")
    )
