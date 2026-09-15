#!/usr/bin/env python3
"""Extract Minecraft default-font glyph advance widths, empirically.

Usage:
    python3 extract-font-metrics.py <minecraft-client.jar> [assets-dir-or-index.json]

Emits a deterministic JSON document on stdout describing:
  * the resolved provider stack for `minecraft:default`,
  * a 95-entry advance table for printable ASCII U+0020..U+007E,
  * the full-width (CJK) advance from the unihex provider, including the
    `size_overrides` that pin it,
  * the half-width unihex advance for a non-CJK codepoint served by unihex.

Nothing outside the Python standard library is used; the PNG decoder is local.

--------------------------------------------------------------------------
WHY THE SECOND ARGUMENT EXISTS
--------------------------------------------------------------------------
Since ~1.20.2 Mojang ships the unihex font data *outside* the client jar. In
1.21.11 the jar's `assets/minecraft/font/include/unifont.json` has an EMPTY
provider list; the real definition and the `unifont.zip` payload are downloaded
objects living in the launcher's asset store, keyed by the version's asset
index. So the ASCII half of this report comes from the jar and the CJK half
comes from the asset store. If the store cannot be located, the ASCII half is
still emitted and the unihex half is reported as unavailable.

--------------------------------------------------------------------------
THE RULES BEING IMPLEMENTED (verified against 1.21.11 client bytecode)
--------------------------------------------------------------------------
Provider priority (FontManager + FontSet):
    FontManager flattens the provider tree by PREPENDING each provider
    (`list.add(0, p)`), then hands `Lists.reverse(list)` to the FontSet. The
    two inversions cancel, so the FontSet iterates in *declaration order* and
    the FIRST provider that supplies a codepoint determines its advance.

BitmapProvider (obf. `goh$a`):
    scale   = providerHeight / cellHeight
    inkCols = (index of rightmost column with alpha != 0) + 1, else 0
    advance = (int)(0.5 + inkCols * scale) + 1
    Only grid cells whose codepoint is 0 are skipped; a *blank* cell still
    yields a glyph, with advance 1. (U+0020 is such a blank cell in ascii.png,
    but the `space` provider is declared first and wins with advance 4.)

UnihexProvider (obf. `gon`):
    A `.hex` line is `CODEPOINT:BITS`; 32 hex digits = 8x16, 64 = 16x16,
    128 = 32x16. Rows are left-aligned in a 32-bit word.
    Measured extents: mask = OR of all 16 rows;
        left  = numberOfLeadingZeros(mask)
        right = 32 - numberOfTrailingZeros(mask) - 1
        (mask == 0 -> left = 0, right = glyphPixelWidth)
    A matching `size_overrides` range REPLACES those extents entirely: the
    loader `remove()`s the codepoint from the measured map before the
    fallback pass runs.
        advance = (right - left + 1) // 2 + 1
"""

import hashlib
import json
import os
import re
import struct
import sys
import zipfile
import zlib

# Vanilla default font options. `uniform` is the "Force Unicode Font" toggle;
# `jp` is the Japanese-variant glyph toggle. Both default to off, which is what
# a normal client renders with.
FONT_OPTIONS = {"uniform": False, "jp": False}

ASCII_FIRST = 0x20
ASCII_LAST = 0x7E

# Representative Han sample (characters drawn from real Simplified Chinese
# game text) plus the full-width punctuation that matters for line budgeting.
HAN_SAMPLE = "无人之洞岛独眼巨序章羊群失踪的牧场火光与影你找到了"
FULLWIDTH_PUNCT_SAMPLE = "，。！？：；（）、“”…—《》"


# ---------------------------------------------------------------------------
# PNG decoding (stdlib only) -> per-pixel alpha
# ---------------------------------------------------------------------------

_PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
_CHANNELS = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}


def _png_chunks(data):
    if data[:8] != _PNG_MAGIC:
        raise ValueError("not a PNG")
    off = 8
    while off + 8 <= len(data):
        (length,) = struct.unpack(">I", data[off:off + 4])
        ctype = data[off + 4:off + 8]
        body = data[off + 8:off + 8 + length]
        yield ctype, body
        off += 12 + length


def _paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    if pb <= pc:
        return b
    return c


def _unfilter(raw, height, stride, fu):
    """Reverse PNG filters 0-4. `fu` is the filter unit in bytes."""
    out = bytearray(height * stride)
    prev = bytearray(stride)
    pos = 0
    for y in range(height):
        ftype = raw[pos]
        pos += 1
        line = bytearray(raw[pos:pos + stride])
        pos += stride
        if ftype == 0:
            pass
        elif ftype == 1:
            for i in range(fu, stride):
                line[i] = (line[i] + line[i - fu]) & 0xFF
        elif ftype == 2:
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 0xFF
        elif ftype == 3:
            for i in range(stride):
                left = line[i - fu] if i >= fu else 0
                line[i] = (line[i] + ((left + prev[i]) >> 1)) & 0xFF
        elif ftype == 4:
            for i in range(stride):
                left = line[i - fu] if i >= fu else 0
                upleft = prev[i - fu] if i >= fu else 0
                line[i] = (line[i] + _paeth(left, prev[i], upleft)) & 0xFF
        else:
            raise ValueError("unknown PNG filter type %d" % ftype)
        out[y * stride:(y + 1) * stride] = line
        prev = line
    return out


def _unpack_samples(row, bit_depth, count):
    """Yield `count` raw sample values from one unfiltered scanline."""
    if bit_depth == 8:
        return list(row[:count])
    if bit_depth == 16:
        # Keep the high byte; alpha significance is unchanged by dropping the low byte.
        return [row[2 * i] for i in range(count)]
    if bit_depth in (1, 2, 4):
        per_byte = 8 // bit_depth
        mask = (1 << bit_depth) - 1
        vals = []
        for i in range(count):
            byte = row[i // per_byte]
            shift = 8 - bit_depth * (i % per_byte + 1)
            vals.append((byte >> shift) & mask)
        return vals
    raise ValueError("unsupported bit depth %d" % bit_depth)


def decode_png_alpha(data):
    """Return (width, height, alpha) where alpha[y][x] is 0..255.

    Mirrors what NativeImage exposes to BitmapProvider: the code tests
    `getLuminanceOrAlpha(x, y) != 0`, which for the RGBA images Minecraft loads
    is the alpha byte.
    """
    width = height = bit_depth = color_type = interlace = None
    palette = b""
    trns = None
    idat = []
    for ctype, body in _png_chunks(data):
        if ctype == b"IHDR":
            (width, height, bit_depth, color_type,
             _compression, _filter, interlace) = struct.unpack(">IIBBBBB", body)
        elif ctype == b"PLTE":
            palette = body
        elif ctype == b"tRNS":
            trns = body
        elif ctype == b"IDAT":
            idat.append(body)
        elif ctype == b"IEND":
            break
    if width is None:
        raise ValueError("PNG has no IHDR")
    if interlace != 0:
        raise ValueError("interlaced PNG is not supported")
    if color_type not in _CHANNELS:
        raise ValueError("unsupported PNG color type %d" % color_type)

    channels = _CHANNELS[color_type]
    raw = zlib.decompress(b"".join(idat))
    bits_per_pixel = channels * bit_depth
    stride = (width * bits_per_pixel + 7) // 8
    filter_unit = max(1, bits_per_pixel // 8)
    flat = _unfilter(raw, height, stride, filter_unit)

    alpha = []
    for y in range(height):
        row = flat[y * stride:(y + 1) * stride]
        samples = _unpack_samples(row, bit_depth, width * channels)
        arow = bytearray(width)
        if color_type == 3:
            for x in range(width):
                idx = samples[x]
                arow[x] = trns[idx] if (trns is not None and idx < len(trns)) else 255
        elif color_type == 0:
            key = None
            if trns is not None and len(trns) >= 2:
                key = struct.unpack(">H", trns[:2])[0] >> (8 if bit_depth == 16 else 0)
            for x in range(width):
                arow[x] = 0 if (key is not None and samples[x] == key) else 255
        elif color_type == 2:
            key = None
            if trns is not None and len(trns) >= 6:
                k = struct.unpack(">HHH", trns[:6])
                key = tuple(v >> (8 if bit_depth == 16 else 0) for v in k)
            for x in range(width):
                px = tuple(samples[3 * x:3 * x + 3])
                arow[x] = 0 if (key is not None and px == key) else 255
        elif color_type == 4:
            for x in range(width):
                arow[x] = samples[2 * x + 1]
        else:  # color_type == 6
            for x in range(width):
                arow[x] = samples[4 * x + 3]
        alpha.append(arow)
    return width, height, alpha


# ---------------------------------------------------------------------------
# Resource lookup: client jar first, launcher asset store second
# ---------------------------------------------------------------------------

class ResourceSource:
    """Resolves `assets/...` paths from the jar, falling back to the asset store."""

    def __init__(self, jar_path, assets_hint=None):
        self.jar_path = jar_path
        self.zip = zipfile.ZipFile(jar_path)
        self.index_path = None
        self.index = {}
        self.assets_root = None
        self.notes = []
        self._locate_assets(jar_path, assets_hint)

    # -- asset store discovery ---------------------------------------------
    def _locate_assets(self, jar_path, hint):
        candidates = []
        if hint:
            candidates.append(hint)
        else:
            # Walk up from <root>/libraries/com/mojang/minecraft/<ver>/x.jar to
            # <root>, where sibling `assets/` lives (PrismLauncher / MultiMC).
            here = os.path.dirname(os.path.abspath(jar_path))
            for _ in range(8):
                cand = os.path.join(here, "assets")
                if os.path.isdir(os.path.join(cand, "indexes")):
                    candidates.append(cand)
                    break
                parent = os.path.dirname(here)
                if parent == here:
                    break
                here = parent
            for extra in (
                os.path.expanduser("~/Library/Application Support/minecraft/assets"),
                os.path.expanduser("~/.minecraft/assets"),
            ):
                if os.path.isdir(os.path.join(extra, "indexes")):
                    candidates.append(extra)
        for cand in candidates:
            if os.path.isfile(cand) and cand.endswith(".json"):
                if self._try_index(cand, os.path.dirname(os.path.dirname(cand))):
                    return
                continue
            if not os.path.isdir(cand):
                continue
            root = cand
            wanted = self._version_asset_index(jar_path, root)
            names = sorted(os.listdir(os.path.join(root, "indexes")))
            if wanted and (wanted + ".json") in names:
                names = [wanted + ".json"] + [n for n in names if n != wanted + ".json"]
            else:
                # Deterministic order: numeric index ids newest-first, then the rest.
                def key(n):
                    stem = n[:-5]
                    return (0, -int(stem)) if stem.isdigit() else (1, stem)
                names = sorted(names, key=key)
            for name in names:
                if self._try_index(os.path.join(root, "indexes", name), root):
                    return
        self.notes.append(
            "asset store not found; unihex (CJK) metrics unavailable. "
            "Pass the assets dir or an index json as the second argument."
        )

    def _version_asset_index(self, jar_path, assets_root):
        """Ask the launcher's version manifest which asset index this jar uses."""
        m = re.search(r"minecraft-(.+?)-client\.jar$", os.path.basename(jar_path))
        if not m:
            return None
        version = m.group(1)
        root = os.path.dirname(assets_root)
        meta = os.path.join(root, "meta", "net.minecraft", version + ".json")
        try:
            with open(meta, encoding="utf-8") as fh:
                data = json.load(fh)
            return str(data["assetIndex"]["id"])
        except Exception:
            return None

    def _try_index(self, index_path, assets_root):
        try:
            with open(index_path, encoding="utf-8") as fh:
                objects = json.load(fh)["objects"]
        except Exception:
            return False
        if "minecraft/font/unifont.zip" not in objects:
            return False
        self.index_path = index_path
        self.index = objects
        self.assets_root = assets_root
        return True

    # -- reads --------------------------------------------------------------
    def read(self, asset_path):
        """Read `assets/minecraft/<rest>`; jar wins, asset store is the fallback."""
        try:
            return self.zip.read(asset_path)
        except KeyError:
            pass
        key = asset_path.split("assets/", 1)[-1]
        entry = self.index.get(key)
        if entry is None:
            raise KeyError(asset_path)
        h = entry["hash"]
        with open(os.path.join(self.assets_root, "objects", h[:2], h), "rb") as fh:
            return fh.read()

    def read_font_json(self, resource_id):
        """Read a font definition, preferring a non-degenerate asset-store copy.

        The 1.21.11 jar ships a stub `include/unifont.json` with zero providers;
        the real one is a downloaded object. Prefer whichever actually declares
        providers, and say so.
        """
        ns, path = split_id(resource_id)
        asset_path = "assets/%s/font/%s.json" % (ns, path)
        key = "%s/font/%s.json" % (ns, path)
        from_jar = None
        try:
            from_jar = json.loads(self.zip.read(asset_path))
        except KeyError:
            pass
        from_store = None
        entry = self.index.get(key)
        if entry is not None:
            h = entry["hash"]
            with open(os.path.join(self.assets_root, "objects", h[:2], h), "rb") as fh:
                from_store = json.loads(fh.read())
        if from_jar is not None and from_store is not None:
            if not from_jar.get("providers") and from_store.get("providers"):
                self.notes.append(
                    "%s: jar copy is an empty stub; using asset-store copy" % resource_id
                )
                return from_store
            return from_jar
        if from_jar is not None:
            return from_jar
        if from_store is not None:
            return from_store
        raise KeyError(resource_id)


def split_id(resource_id):
    if ":" in resource_id:
        ns, path = resource_id.split(":", 1)
    else:
        ns, path = "minecraft", resource_id
    return ns, path


# ---------------------------------------------------------------------------
# Providers
# ---------------------------------------------------------------------------

class SpaceProvider:
    kind = "space"

    def __init__(self, defn):
        self.advances = {ord(k): v for k, v in defn["advances"].items()}
        self.describe = {"type": "space", "advances": len(self.advances)}

    def has(self, cp):
        return cp in self.advances

    def advance(self, cp):
        return self.advances[cp]


class BitmapProviderImpl:
    kind = "bitmap"

    def __init__(self, defn, source):
        self.file = defn["file"]
        self.height = defn.get("height", 8)
        self.ascent = defn["ascent"]
        rows = defn["chars"]
        ns, path = split_id(self.file)
        png = source.read("assets/%s/textures/%s" % (ns, path))
        self.img_w, self.img_h, alpha = decode_png_alpha(png)
        self.grid_cols = len(rows[0])
        self.grid_rows = len(rows)
        self.cell_w = self.img_w // self.grid_cols
        self.cell_h = self.img_h // self.grid_rows
        self.scale = self.height / self.cell_h
        self._adv = {}
        for r, row in enumerate(rows):
            for c, ch in enumerate(row):
                cp = ord(ch)
                if cp == 0:            # BitmapProvider skips codepoint 0 only
                    continue
                if cp in self._adv:    # first cell wins; MC warns and keeps the last,
                    continue           # but vanilla sheets have no duplicates
                self._adv[cp] = self._compute(r, c, alpha)
        self.describe = {
            "type": "bitmap",
            "file": self.file,
            "height": self.height,
            "ascent": self.ascent,
            "png_size": [self.img_w, self.img_h],
            "grid": [self.grid_rows, self.grid_cols],
            "cell": [self.cell_w, self.cell_h],
            "scale": self.scale,
            "glyphs": len(self._adv),
        }

    def _compute(self, r, c, alpha):
        ink = 0
        for i in range(self.cell_w - 1, -1, -1):
            x = c * self.cell_w + i
            found = False
            for k in range(self.cell_h):
                if alpha[r * self.cell_h + k][x] != 0:
                    found = True
                    break
            if found:
                ink = i + 1
                break
        return int(0.5 + ink * self.scale) + 1, ink

    def has(self, cp):
        return cp in self._adv

    def advance(self, cp):
        return self._adv[cp][0]

    def ink(self, cp):
        return self._adv[cp][1]


class UnihexProviderImpl:
    kind = "unihex"

    def __init__(self, defn, source):
        self.hex_file = defn["hex_file"]
        raw_overrides = defn.get("size_overrides", [])
        self.overrides = [
            (ord(o["from"]), ord(o["to"]), o["left"], o["right"]) for o in raw_overrides
        ]
        ns, path = split_id(self.hex_file)
        blob = source.read("assets/%s/%s" % (ns, path))
        self.lines = {}
        self.entries = []
        zf = zipfile.ZipFile(__import__("io").BytesIO(blob))
        for name in zf.namelist():
            if not name.endswith(".hex"):
                continue
            self.entries.append(name)
            for raw in zf.read(name).decode("ascii").splitlines():
                raw = raw.strip()
                if not raw or ":" not in raw:
                    continue
                cps, bits = raw.split(":", 1)
                cp = int(cps, 16)
                n = len(bits)
                if n % 16 != 0:
                    raise ValueError("bad hex payload length %d at U+%04X" % (n, cp))
                width = (n // 16) * 4          # hex digits per row * 4 bits
                per = n // 16
                rows = []
                for r in range(16):
                    v = int(bits[r * per:(r + 1) * per], 16)
                    rows.append((v << (32 - width)) & 0xFFFFFFFF)
                self.lines[cp] = (width, rows)
        self.describe = {
            "type": "unihex",
            "hex_file": self.hex_file,
            "hex_entries": sorted(self.entries),
            "codepoints": len(self.lines),
            "size_overrides": [
                {"from": "U+%04X" % f, "to": "U+%04X" % t, "left": l, "right": r}
                for f, t, l, r in self.overrides
            ],
        }

    def has(self, cp):
        return cp in self.lines

    def measured_extents(self, cp):
        width, rows = self.lines[cp]
        mask = 0
        for v in rows:
            mask |= v
        if mask == 0:
            return 0, width, width          # blank glyph: right = glyph pixel width
        left = 32 - mask.bit_length()                    # numberOfLeadingZeros
        ntz = (mask & -mask).bit_length() - 1            # numberOfTrailingZeros
        return left, 32 - ntz - 1, width

    def override_for(self, cp):
        for f, t, l, r in self.overrides:
            if f <= cp <= t:
                return l, r
        return None

    def extents(self, cp):
        ov = self.override_for(cp)
        if ov is not None:
            return ov
        left, right, _ = self.measured_extents(cp)
        return left, right

    def advance(self, cp):
        left, right = self.extents(cp)
        return (right - left + 1) // 2 + 1

    def glyph_width(self, cp):
        return self.lines[cp][0]


# ---------------------------------------------------------------------------
# Provider stack assembly
# ---------------------------------------------------------------------------

def build_stack(source, font_id="minecraft:default"):
    """Flatten the font's provider tree into effective priority order.

    FontManager prepends each provider while walking, then reverses the result;
    the net effect is declaration order, first-wins. We build declaration order
    directly.
    """
    out = []
    seen = []

    def walk(fid):
        if fid in seen:
            raise ValueError("reference cycle at %s" % fid)
        seen.append(fid)
        defn = source.read_font_json(fid)
        for prov in defn.get("providers", []):
            flt = prov.get("filter", {})
            if any(FONT_OPTIONS.get(k) != v for k, v in flt.items()):
                continue
            ptype = prov["type"]
            if ptype == "reference":
                walk(prov["id"])
            elif ptype == "space":
                out.append(SpaceProvider(prov))
            elif ptype == "bitmap":
                out.append(BitmapProviderImpl(prov, source))
            elif ptype == "unihex":
                out.append(UnihexProviderImpl(prov, source))
            elif ptype == "ttf":
                out.append(None)  # not used by vanilla default; placeholder
            else:
                raise ValueError("unknown provider type %r" % ptype)
        seen.pop()

    walk(font_id)
    return [p for p in out if p is not None]


def resolve(stack, cp):
    """First provider in priority order that supplies `cp` wins."""
    for i, prov in enumerate(stack):
        if prov.has(cp):
            return i, prov
    return None, None


# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------

def main(argv):
    if not (2 <= len(argv) <= 3):
        sys.stderr.write(__doc__.split("---")[0].strip() + "\n")
        return 2
    jar = argv[1]
    hint = argv[2] if len(argv) == 3 else None
    if not os.path.isfile(jar):
        sys.stderr.write("error: cannot read jar: %s\n" % jar)
        return 2
    try:
        source = ResourceSource(jar, hint)
        stack = build_stack(source)
    except Exception as exc:                       # noqa: BLE001 - report and fail
        sys.stderr.write("error: %s: %s\n" % (type(exc).__name__, exc))
        return 2

    with open(jar, "rb") as fh:
        jar_sha1 = hashlib.sha1(fh.read()).hexdigest()

    report = {
        "schema": "delvewright/minecraft-font-metrics/1",
        "source": {
            "jar": os.path.basename(jar),
            "jar_sha1": jar_sha1,
            "asset_index": (os.path.basename(source.index_path)
                            if source.index_path else None),
            "font_options": dict(sorted(FONT_OPTIONS.items())),
        },
        "notes": sorted(set(source.notes)),
        "providers": [p.describe for p in stack],
    }

    # -- 1. ASCII table -----------------------------------------------------
    table = []
    detail = []
    for cp in range(ASCII_FIRST, ASCII_LAST + 1):
        i, prov = resolve(stack, cp)
        if prov is None:
            raise SystemExit("no provider supplies U+%04X" % cp)
        adv = prov.advance(cp)
        table.append(adv)
        entry = {
            "cp": "U+%04X" % cp,
            "char": chr(cp),
            "advance": adv,
            "provider": i,
            "provider_type": prov.kind,
        }
        if isinstance(prov, BitmapProviderImpl):
            entry["ink_columns"] = prov.ink(cp)
            entry["file"] = prov.file
        detail.append(entry)

    counts = {}
    for a in table:
        counts[a] = counts.get(a, 0) + 1
    report["ascii"] = {
        "range": ["U+%04X" % ASCII_FIRST, "U+%04X" % ASCII_LAST],
        "note": "index = codepoint - 0x20",
        "advances": table,
        "histogram": {str(k): counts[k] for k in sorted(counts)},
        "non_six": {"U+%04X" % cp: a
                    for cp, a in zip(range(ASCII_FIRST, ASCII_LAST + 1), table)
                    if a != 6},
        "detail": detail,
    }

    # -- 2/3. unihex --------------------------------------------------------
    uni = next((p for p in stack if isinstance(p, UnihexProviderImpl)), None)
    if uni is None:
        report["unihex"] = {"available": False,
                            "reason": "no unihex provider resolved (asset store missing)"}
    else:
        def sample(text):
            rows = []
            for ch in text:
                cp = ord(ch)
                i, prov = resolve(stack, cp)
                row = {"cp": "U+%04X" % cp, "char": ch,
                       "provider": i,
                       "provider_type": prov.kind if prov else None}
                if isinstance(prov, UnihexProviderImpl):
                    ml, mr, gw = prov.measured_extents(cp)
                    ov = prov.override_for(cp)
                    row.update({
                        "glyph_pixel_width": gw,
                        "measured_left": ml, "measured_right": mr,
                        "measured_advance": (mr - ml + 1) // 2 + 1,
                        "size_override": list(ov) if ov else None,
                        "advance": prov.advance(cp),
                    })
                elif prov is not None:
                    row["advance"] = prov.advance(cp)
                rows.append(row)
            return rows

        han_rows = sample(HAN_SAMPLE)
        punct_rows = sample(FULLWIDTH_PUNCT_SAMPLE)

        han_adv = sorted({r["advance"] for r in han_rows})
        # Every CJK Unified Ideograph present, for a population-level check.
        all_han = [cp for cp in uni.lines if 0x4E00 <= cp <= 0x9FFF]
        all_han_adv = {}
        for cp in all_han:
            a = uni.advance(cp)
            all_han_adv[a] = all_han_adv.get(a, 0) + 1

        # Non-CJK codepoints served by unihex with a half-width (8px) glyph.
        # Skip blank glyphs: for those the loader sets right = glyphPixelWidth
        # rather than a real ink extent, which is a degenerate case.
        halfwidth = []
        for cp in sorted(uni.lines):
            if len(halfwidth) >= 4:
                break
            if cp < 0x2000 or uni.glyph_width(cp) != 8:
                continue
            if uni.override_for(cp) is not None:
                continue
            ml, mr, gw = uni.measured_extents(cp)
            if ml == 0 and mr == gw:        # blank glyph
                continue
            i, prov = resolve(stack, cp)
            if prov is not uni:
                continue
            halfwidth.append({"cp": "U+%04X" % cp, "char": chr(cp),
                              "glyph_pixel_width": gw,
                              "measured_left": ml, "measured_right": mr,
                              "advance": uni.advance(cp)})

        report["unihex"] = {
            "available": True,
            "advance_formula": "(right - left + 1) // 2 + 1",
            "han_sample": han_rows,
            "han_sample_distinct_advances": han_adv,
            "cjk_unified_ideographs_present": len(all_han),
            "cjk_unified_ideographs_advance_histogram":
                {str(k): all_han_adv[k] for k in sorted(all_han_adv)},
            "fullwidth_punctuation_sample": punct_rows,
            "halfwidth_examples": halfwidth,
        }

        # Gotchas: characters that appear in CJK copy but are served by a
        # *bitmap* provider declared ahead of unihex, so they are NOT 9 wide.
        gotchas = []
        for ch in "“”‘’…—–·《》〈〉「」『』【】":
            cp = ord(ch)
            i, prov = resolve(stack, cp)
            if prov is not None and not isinstance(prov, UnihexProviderImpl):
                gotchas.append({"cp": "U+%04X" % cp, "char": ch,
                                "provider": i, "provider_type": prov.kind,
                                "file": getattr(prov, "file", None),
                                "advance": prov.advance(cp)})
        report["cjk_text_gotchas"] = {
            "note": ("these occur in CJK copy but resolve to a bitmap provider "
                     "declared before unihex, so their advance is not the "
                     "full-width 9"),
            "entries": gotchas,
        }

    # -- bottom line --------------------------------------------------------
    lower = [table[ord(c) - ASCII_FIRST] for c in "abcdeghmnopqrsuvwxyz"]
    upper = [table[ord(c) - ASCII_FIRST] for c in "ABCDEFGHJKLMNOPQRSTUVWXYZ"]
    bottom = {
        "latin_lowercase_typical": max(set(lower), key=lower.count),
        "latin_uppercase_typical": max(set(upper), key=upper.count),
        "space": table[0],
    }
    if uni is not None:
        bottom["han_typical"] = han_adv[0] if len(han_adv) == 1 else han_adv
        pa = sorted({r["advance"] for r in punct_rows if r["provider_type"] == "unihex"})
        bottom["fullwidth_punctuation_typical"] = pa[0] if len(pa) == 1 else pa
        if isinstance(bottom["han_typical"], int):
            bottom["han_to_latin_ratio"] = "%d:%d" % (
                bottom["han_typical"], bottom["latin_lowercase_typical"])
    report["bottom_line"] = bottom

    json.dump(report, sys.stdout, indent=2, ensure_ascii=False, sort_keys=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
