#!/usr/bin/env python3
"""The storybook illustration path runs, on every horizon the engine can build.

## The gap this closes

`delvec panorama` and `delvec scene` read the compiler's own `render-plan.json`
and turn it into Chunky scenes — the release illustration and the whole-scene
review renders. **Nothing in this repository invoked either of them.**
`tools/ci/check-gallery-render.py` runs `snapshot`, and only on the primary
gallery.

What that cost, measured rather than supposed: the render layer's `Horizon` enum
carried ONE variant while the compiler had been emitting a second for as long as
the base existed, and `#[serde(tag = "kind")]` has no fallback — so one unknown
value failed the whole document and **both commands refused every valley
campaign outright** with

    DW0721 [error] parse render-plan.json: unknown variant `valley`, expected `ocean`

A producer and a consumer of one document, and nothing compared them. That is
the UNRUN vacuity mode in the release path: a gate whose obligation lived in
nobody's head at all.

## What it asserts, and why the enumeration is the point

A check that ran the arms on whatever build happened to be lying around would
have passed for years — the primary gallery declares the default horizon, and
the default horizon is exactly the one that worked. So the population is fixed
by the ENGINE rather than by this file: the horizon bases are read from
`delvec schema --stage all`, which is the same single authority the coverage
gate enumerates its units from, and every one of them must be exercised.

A base with no gallery point declaring it is a red naming the base. A base whose
point exists and whose arms refuse is a red naming the command and the reason.
There is no third state and no list here to go stale — adding a base to the
engine reds this file until something renders it.

Per point, for each of `scene` and `panorama`:

- the command exits 0, and its stderr is reported verbatim when it does not;
- it emits at least one scene file;
- **the frame contains the ground.** A scene's `chunkList` must cover the
  landform the horizon declares, not merely the layout. The layout AABB is the
  union of the placed AREAS, so on a horizon that BUILDS terrain a chunk list
  solved from it alone loads none of the landform the place stands in. That
  defect renders successfully and produces a picture, which is why it needs an
  assertion rather than an eye.
- **the panorama frames the built place, and fills the frame with it.** Read
  the way Chunky reads the scene — `camera.orientation` in Chunky's basis,
  `fov` as the VERTICAL field of view, the horizontal half-extent scaled by
  `width / height` — every corner of the plan's `layout_aabb` must project
  inside the frame, and the rectangle they span must cover at least a third of
  it. Framing the union of the layout and the landform instead is the defect this
  catches: on a valley it put the castle a fifth of the frame wide and filled the
  rest with slopes and the void past them. The projection here is a second
  implementation, sharing no code with the engine's solve.

And for `delvec cameras`, on every point whose campaign carries a
`design/cameras.json`:

- **every stated camera is the scene's camera.** One scene per camera, named
  `<campaign>_camera_<name>`, whose position, vertical `fov`, frame and sample
  target are the record's, whose exposure is the engine's derived 1.0 (the
  record states none), and whose view direction — derived from the
  stored orientation in Chunky's basis — is the direction Minecraft looks for the
  record's own yaw and pitch (`(-sin yaw·cos pitch, -sin pitch, cos yaw·cos
  pitch)`), within 1e-5. That formula is written here from the game's convention,
  sharing nothing with the engine's conversion, so a yaw read with the wrong sign
  or the wrong zero is a red rather than a picture of the wrong wall.
- the record binds: a campaign whose record states no camera, or a run that
  judged zero cameras over the whole domain, is a red.
- **every camera is under its picture's sky** (spec-0079): the camera's own
  `sky`, else the `time` and `weather` of the `design.json` row it answers —
  read here from the two documents, sharing nothing with the engine's
  resolution. Every scene carries the **game's own sky of its tick under its
  weather** (`compiler.md` §4): its `delvewrightSky.daytimeTicks` is held within
  `TICK_SLACK` ticks of the tick its own sun names by minecraft.wiki's
  closed-form sky angle, which the engine's emission (the vendored track) does
  not share; the fixed keys are `SKYMAP_EQUIRECTANGULAR`, `skyMirrored` false,
  `apparentSkyLight` 1, no fog, no sun light and the disc drawn exactly while
  some of it stands above the horizon under a clear sky (never under rain or
  thunder); the marker names the weather, absent under a clear sky; the map
  written beside the scene is held, on a 24 × 12 sample grid, to this gate's
  own Python port of the client's sky and its weather layer (the clear colour
  with its look-dependent sunrise tint, lerped toward the sky colour the
  weather darkens, the octagonal sky disc with `sky.fsh`'s fog, the sunrise
  fan, every attribute through `WeatherAttributes`' map) plus ONE uniform
  non-negative veil; and `skyLight` times the map's cosine-weighted mean over
  the upper hemisphere is held to the lightmap shader's full-sky light of
  `visual/sky_light_factor` and `visual/sky_light_color` under the weather. The
  panorama and review scenes of a plan are judged by the same rule. Over the
  domain the weathers emitted are `{clear, rain, thunder}` and the sun bands —
  read off each scene's sun altitude, at or above 20° `high`, from 0° `low`,
  under 0° `below` — are `{high, low, below}`; a domain reaching fewer is a
  red, because the gallery owes every weather and every hour's band a frame.

And on every scene of every arm: **mid-grey shows as the game shows it.** The
scene's `postprocess` is `TONEMAP3` and its `postprocessSettings` are the seven
floats the pinned core's `HableToneMappingFilter` reads; evaluated here by this
gate's own port of the core's arithmetic (`processPixel` and
`recalculateWhiteScale`), linear 0.18 shows within 1e-4 of `0.18^(1/2.2)`, the
game's transfer of a decoded colour (`compiler.md` §4), and the white point
shows white. Settings absent, a key missing, or the core's bare preset (which
shows 0.18 at 0.393) is a red. **And its emitters carry the lantern scale**:
`emitterIntensity` is the core's default 13 (`Scene.DEFAULT_EMITTER_INTENSITY`)
times the scale `compiler.md` §4 measured, 12.2310 — fitted on frames that
`validation/chunky.sh` denoises after every `-render` — within 1e-6; on a
night-vision emulation every overridden `emittance` times that intensity is the
calibrated 0.05 × 13, so the emulation's glow does not move with the lanterns.

The arms refuse a build with no world save (Chunky renders a missing world as an
empty frame at exit 0). This gate judges scene bytes and renders nothing, so each
build gets a stub save — `level.dat` and one region file — which is exactly the
shape the arms check for and nothing Chunky could load.

## Binding count

Every run states the bases the schema declares, the points found for them, the
builds exercised, and the scene files judged. A base unaccounted for is a red;
zero scenes judged is a red. A gate that rendered nothing is vacuous, not a
pass.
"""

from __future__ import annotations

import argparse
import contextlib
import json
import math
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
import parallel  # noqa: E402
from delvec_bin import resolve as resolve_delvec  # noqa: E402
from gallery_domain import GALLERY, materialise, overlays  # noqa: E402

REPO = Path(__file__).resolve().parents[2]


def die(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def declared_bases(delvec: Path) -> list[str]:
    """Every horizon base the ENGINE declares, from its own schema export."""
    r = subprocess.run(
        [str(delvec), "schema", "--stage", "all"], capture_output=True, text=True
    )
    if r.returncode != 0:
        die(f"`delvec schema --stage all` exited {r.returncode}: {r.stderr.strip()}")
    doc = json.loads(r.stdout)
    found: list[str] = []

    def walk(node: object) -> None:
        if isinstance(node, dict):
            one_of = node.get("oneOf")
            if isinstance(one_of, list):
                consts = [e.get("const") for e in one_of if isinstance(e, dict)]
                if consts and all(isinstance(c, str) for c in consts) and "void" in consts:
                    found.extend(consts)
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for v in node:
                walk(v)

    walk(doc)
    if not found:
        die(
            "the schema export declares no horizon base enumeration. The population "
            "this gate quantifies over comes from the engine and from nowhere else, "
            "so an empty one is a broken reading, not an empty world."
        )
    return sorted(set(found))


def horizon_of(world_json: Path) -> str | None:
    """The base a stage-1 document declares, or `None` if it declares none."""
    if not world_json.is_file():
        return None
    doc = json.loads(world_json.read_text())
    h = doc.get("content", doc).get("horizon")
    if h is None:
        return None
    return h if isinstance(h, str) else h.get("base")


def points_by_base() -> dict[str, str | None]:
    """Which gallery point declares each base. Derived, never listed."""
    out: dict[str, str | None] = {}
    primary = horizon_of(GALLERY / "world.json")
    if primary:
        out.setdefault(primary, None)
    for name in overlays():
        base = horizon_of(GALLERY / "overlays" / name / "world.json")
        if base:
            out.setdefault(base, name)
    return out


def run(cmd: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True)


def covers(chunks: list[list[int]], lo: list[int], hi: list[int]) -> list[str]:
    """Chunk columns of `[lo, hi]` that the list is missing."""
    have = {(c[0], c[1]) for c in chunks}
    missing = []
    for cx in range(lo[0] // 16, hi[0] // 16 + 1):
        for cz in range(lo[2] // 16, hi[2] // 16 + 1):
            if (cx, cz) not in have:
                missing.append(f"[{cx}, {cz}]")
    return missing


def stub_world(build: Path) -> None:
    """The shape of a world save the arms accept; see the module docs."""
    region = build / "world" / "region"
    region.mkdir(parents=True, exist_ok=True)
    (build / "world" / "level.dat").write_bytes(b"stub")
    (region / "r.0.0.mca").write_bytes(b"stub")


# The exposure every scene is written at: the one value the sky, the tone curve
# and the lantern scale are calibrated at (`compiler.md` §4). Typed here from the
# reference, not read from the engine, so a scene the engine exposes otherwise
# is a red.
DERIVED_EXPOSURE = 1.0


def camera_matches(doc: dict, cam: dict) -> list[str]:
    """What differs between a stated camera and the scene emitted for it."""
    bad = []
    c = doc["camera"]
    pos = (c["position"]["x"], c["position"]["y"], c["position"]["z"])
    if any(abs(pos[k] - cam["pos"][k]) > 1e-5 for k in range(3)):
        bad.append(f"position {pos} is not the record's {cam['pos']}")
    for key, want in (("fov", cam["fov"]),):
        if abs(c[key] - want) > 1e-5:
            bad.append(f"{key} {c[key]} is not the record's {want}")
    for key, field in (("width", "width"), ("height", "height"), ("sppTarget", "spp")):
        if doc[key] != cam[field]:
            bad.append(f"{key} {doc[key]} is not the record's {cam[field]}")
    if "exposure" in cam:
        bad.append(f"the record states `exposure` {cam['exposure']}, which the engine derives")
    if abs(doc["exposure"] - DERIVED_EXPOSURE) > 1e-5:
        bad.append(f"exposure {doc['exposure']} is not the derived {DERIVED_EXPOSURE}")
    yaw, pitch = c["orientation"]["yaw"], c["orientation"]["pitch"]
    got = (math.cos(yaw) * math.sin(pitch), -math.cos(pitch), -math.sin(yaw) * math.sin(pitch))
    y, p = math.radians(cam["yaw"]), math.radians(cam["pitch"])
    want = (-math.sin(y) * math.cos(p), -math.sin(p), math.cos(y) * math.cos(p))
    if any(abs(got[k] - want[k]) > 1e-5 for k in range(3)):
        bad.append(
            f"looks along {tuple(round(v, 4) for v in got)}, where Minecraft yaw "
            f"{cam['yaw']} pitch {cam['pitch']} looks along {tuple(round(v, 4) for v in want)}"
        )
    return bad


# The share of the frame a panorama's subject must cover. A third is what the
# island's accepted release art measures: its built place's bounding rectangle
# over its frame.
# The pinned game's day timeline, vendored for the engine; the clear cell a
# clear scene carries is derived from it here, with a reader of this gate's own.
DAY_TIMELINE = REPO / "crates/dsl/data/timeline-day-1.21.11.json"

# Cited from the pinned jars, read here and not taken from the engine: the
# ground biomes' `visual/sky_color` (`the_void` and `ocean` both `#7ba4ff`), the
# overworld dimension's `visual/fog_color` (`#c0d8ff`), and the client's
# `gamma` option default (0.5), the lightmap's brightness factor.
GROUND_SKY = (0x7B, 0xA4, 0xFF)
OVERWORLD_FOG = (0xC0, 0xD8, 0xFF)
BRIGHTNESS = 0.5
# Half the angle the sun's drawn disc spans: `atan(30 × 8/32 / 100)`.
SUN_DISC_HALF_DEG = math.degrees(math.atan(30 * 8 / 32 / 100))
# The pinned client's weather layer (`WeatherAttributes.<clinit>`, read off its
# bytecode through Mojang's client mappings), ported here on its own: per
# weather, `blend_to_gray` (brightness, factor) on the sky colour, a `multiply`
# on the fog and sunrise colours (`ARGB.colorFromFloat` floats), and the alpha
# both `alpha_blend`s take toward `visual/sky_light_factor` 0.24 and
# `NIGHT_SKY_LIGHT_COLOR` (1, 0.48, 0.48, 1). A `thunder` delve stands at rain
# and thunder level 1, so its map is applied alone; a `rain` delve's is.
WEATHER_LAYER = {
    "rain": {"gray": (0.6, 0.75), "darken": (0.5, 0.5, 0.6), "alpha": 0.3125},
    "thunder": {"gray": (0.24, 0.94), "darken": (0.25, 0.25, 0.3), "alpha": 0.52734375},
}
WEATHER_LEVELS = {"clear": (0.0, 0.0), "rain": (1.0, 0.0), "thunder": (1.0, 1.0)}
# How many ticks either side of the tick a scene's sun names by the closed form
# the cell may come from: the closed form and the pinned track agree to 0.056°,
# a few ticks where the sun is lowest.
TICK_SLACK = 12


def _track(name: str, timeline: Path = DAY_TIMELINE) -> list[dict]:
    tr = json.loads(timeline.read_text())["tracks"][f"minecraft:visual/{name}"]
    if tr.get("ease") is not None:
        die(f"{timeline}: `visual/{name}` declares an ease this gate does not read")
    return tr["keyframes"]


def _between(kf: list[dict], tick: int) -> tuple[object, object, float]:
    t = tick % 24000
    idx = [i for i, k in enumerate(kf) if k["ticks"] <= t]
    if idx:
        i = idx[-1]
        a = kf[i]
        b, bt = (kf[i + 1], kf[i + 1]["ticks"]) if i + 1 < len(kf) else (kf[0], kf[0]["ticks"] + 24000)
        at = a["ticks"]
    else:
        a, b = kf[-1], kf[0]
        at, bt = a["ticks"] - 24000, b["ticks"]
    p = 0.0 if bt <= at else (t - at) / (bt - at)
    return a["value"], b["value"], p


def timeline_rgb(name: str, tick: int) -> tuple[int, int, int]:
    """A colour track at a tick, each 8-bit channel `a + floor(p·(b−a))`."""
    a, b, p = _between(_track(name), tick)
    ca = [int(str(a)[1 + 2 * i : 3 + 2 * i], 16) for i in range(3)]
    cb = [int(str(b)[1 + 2 * i : 3 + 2 * i], 16) for i in range(3)]
    return tuple(x + math.floor(p * (y - x)) for x, y in zip(ca, cb))  # type: ignore[return-value]


def timeline_float(name: str, tick: int) -> float:
    a, b, p = _between(_track(name), tick)
    return float(a) + (float(b) - float(a)) * p


def closed_form_altitude(tick: float) -> float:
    """The sun's altitude in degrees at a `daytime` tick, by minecraft.wiki's
    closed-form sky angle (*Daylight cycle*, §Sky angle) — not the vendored
    track the engine reads."""
    t = tick - 6000.0
    turn = (t / 24000.0) % 1.0
    quarters = (t / 6000.0) % 4.0
    angle = ((1.0 - math.cos(math.pi * turn) + quarters) * 60.0) % 360.0
    return 90.0 - angle if angle <= 180.0 else angle - 270.0


def tick_of_sun(altitude_rad: float, azimuth_rad: float) -> int:
    """The `daytime` tick whose closed-form sun stands at a scene's altitude, on
    the side its azimuth names (π = west, the afternoon half 6000..18000)."""
    alt = math.degrees(altitude_rad)
    west = abs(azimuth_rad - math.pi) < 1e-3
    lo, hi = (6000.0, 18000.0) if west else (18000.0, 30000.0)
    # Altitude falls monotonically from noon to midnight and rises back.
    for _ in range(80):
        mid = 0.5 * (lo + hi)
        above = closed_form_altitude(mid) > alt
        if above == west:
            lo = mid
        else:
            hi = mid
    return int(round(0.5 * (lo + hi))) % 24000


def timeline_argb(name: str, tick: int) -> tuple[int, int, int, int]:
    """An `#aarrggbb` track at a tick, each 8-bit channel `a + floor(p·(b−a))`."""
    a, b, p = _between(_track(name), tick)
    ca = [int(str(a)[1 + 2 * i : 3 + 2 * i], 16) for i in range(4)]
    cb = [int(str(b)[1 + 2 * i : 3 + 2 * i], 16) for i in range(4)]
    return tuple(x + math.floor(p * (y - x)) for x, y in zip(ca, cb))  # type: ignore[return-value]


def _mul(a, b):
    return [x * y // 255 for x, y in zip(a, b)]


def _floor8(f: float) -> int:
    """`ARGB.as8BitChannel`: floor(f x 255) in float."""
    return math.floor(_f32(_f32(f) * 255.0))


def weather_attrs(tick: int, weather: str) -> dict:
    """The sky attributes the client reads at a tick under a weather: the
    timeline's, over the ground biome, with the weather's layer applied."""
    at = {
        "factor": timeline_float("sky_light_factor", tick),
        "light": list(timeline_rgb("sky_light_color", tick)),
        "sky": _mul(timeline_rgb("sky_color", tick), GROUND_SKY),
        "fog": _mul(timeline_rgb("fog_color", tick), OVERWORLD_FOG),
        "sunrise": list(timeline_argb("sunrise_sunset_color", tick)),
    }
    layer = WEATHER_LAYER.get(weather)
    if layer is None:
        return at
    a = _f32(layer["alpha"])
    f = _f32(at["factor"])
    m = _f32(f + _f32(a * _f32(_f32(0.24) - f)))
    at["factor"] = _f32(f + _f32(m - f))  # the layer's own lerp at weight 1
    sa = _floor8(a)
    night = [_floor8(0.48), _floor8(0.48), _floor8(1.0)]
    at["light"] = [(n * sa + c * (255 - sa)) // 255 for n, c in zip(night, at["light"])]
    bright, factor = layer["gray"]
    r, g, b = at["sky"]
    grey = int(_f32(_f32(_f32(r * _f32(0.3)) + _f32(g * _f32(0.59))) + _f32(b * _f32(0.11))))
    scaled = min(255, max(0, int(_f32(grey * _f32(bright)))))
    at["sky"] = [c + math.floor(_f32(_f32(factor) * (scaled - c))) for c in at["sky"]]
    darken = [_floor8(c) for c in layer["darken"]]
    at["fog"] = _mul(at["fog"], darken)
    at["sunrise"] = [at["sunrise"][0]] + _mul(at["sunrise"][1:], darken)
    return at


def weather_darken(c, weather: str) -> list[int]:
    """`AtmosphericFogEnvironment.applyWeatherDarken` on the sky colour the
    clear colour is lerped toward."""
    rain, thunder = WEATHER_LEVELS[weather]
    c = list(c)
    if rain > 0:
        k = (_f32(1 - rain * 0.5), _f32(1 - rain * 0.5), _f32(1 - rain * 0.4))
        c = [min(255, max(0, int(_f32(v * f)))) for v, f in zip(c, k)]
    if thunder > 0:
        k2 = _f32(1 - thunder * 0.5)
        c = [min(255, max(0, int(_f32(v * k2)))) for v in c]
    return c


def lightmap_light(tick: int, weather: str = "clear") -> list[float]:
    """The lightmap shader's full-sky colour (`lightmap.fsh`, sky light 15,
    block light 0, the gamma option at 0.5), decoded by Python's `**`: the light
    the sky casts on an open top face at a tick under a weather."""
    at = weather_attrs(tick, weather)
    factor = at["factor"]
    light = at["light"]
    c = [min(1.0, max(0.0, (v / 255.0 * factor) * 0.96 + 0.03)) for v in light]
    top = max(c)
    curve = 1.0 - (1.0 - top) ** 4
    lit = [(v + (v * curve / top - v) * BRIGHTNESS) * 0.96 + 0.03 for v in c]
    return [v**2.2 for v in lit]


# The client's defaults and the sky geometry its `SkyRenderer` builds, read
# here off the same bytecode the engine cites: render distance 12 chunks,
# `visual/sky_fog_end_distance` 512, the sky disc an octagon at height 16 with
# its rim at 512, the sunrise fan from (0, 100, 0) to a rim of radius 120 lifted
# 40 × alpha, 16 steps. The map is 360 × 180, rows from the nadir up.
RENDER_DISTANCE = 12
SKY_FOG_END = 512.0
MAP_W, MAP_H = 360, 180


def game_sky_colours(tick: int, view: tuple[float, float, float], weather: str = "clear") -> dict:
    """The clear colour, the sky colour and the fan of a tick under a weather
    for a look vector, as display values — `AtmosphericFogEnvironment.
    getBaseColor`'s steps: the clear colour is lerped toward the sky colour the
    weather darkens, the disc is drawn in the sky colour itself."""
    at = weather_attrs(tick, weather)
    fog = at["fog"]
    sky = at["sky"]
    toward = weather_darken(sky, weather)
    a, r, g, b = at["sunrise"]
    alpha = a / 255.0
    angle = closed_form_angle(tick)
    west = 0.0 < angle < 180.0
    side = -1.0 if west else 1.0
    dot = view[0] * side
    lerp = lambda f, x, y: [u + math.floor(f * (v - u)) for u, v in zip(x, y)]  # noqa: E731
    if RENDER_DISTANCE >= 4 and dot > 0 and alpha > 0:
        fog = lerp(dot * alpha, fog, [r, g, b])
    reach = min(max(min(SKY_FOG_END / 16, RENDER_DISTANCE) / 32, 0.25), 1.0)
    fog = lerp(1 - reach**0.25, fog, toward)
    return {
        "fog": [v / 255 for v in fog],
        "sky": [v / 255 for v in sky],
        "fan": ([r / 255, g / 255, b / 255], alpha, side) if alpha > 0.001 else None,
    }


def closed_form_angle(tick: float) -> float:
    """The sun angle in degrees from the zenith, westward, by the closed form."""
    t = tick - 6000.0
    turn = (t / 24000.0) % 1.0
    quarters = (t / 6000.0) % 4.0
    return ((1.0 - math.cos(math.pi * turn) + quarters) * 60.0) % 360.0


def _fan_alpha(fan, d) -> float:
    rgb, alpha, s = fan
    c = (100.0 * s, 0.0, 0.0)

    def rim(j):
        a = 2 * math.pi * j / 16
        return (120.0 * s * math.cos(a), 40.0 * alpha * math.cos(a), -120.0 * s * math.sin(a))

    def sub(p, q):
        return (p[0] - q[0], p[1] - q[1], p[2] - q[2])

    def cross(p, q):
        return (p[1] * q[2] - p[2] * q[1], p[2] * q[0] - p[0] * q[2], p[0] * q[1] - p[1] * q[0])

    def dot(p, q):
        return p[0] * q[0] + p[1] * q[1] + p[2] * q[2]

    for j in range(16):
        e1, e2 = sub(rim(j), c), sub(rim(j + 1), c)
        p = cross(d, e2)
        det = dot(e1, p)
        if abs(det) < 1e-12:
            continue
        tv = (-c[0], -c[1], -c[2])
        u = dot(tv, p) / det
        if not 0 <= u <= 1:
            continue
        q = cross(tv, e1)
        v = dot(d, q) / det
        if v < 0 or u + v > 1:
            continue
        if dot(e2, q) / det > 0:
            return alpha * (1 - u - v)
    return 0.0


def game_sky_pixel(colours: dict, d) -> list[float]:
    """What the client draws along `d`, decoded to linear."""
    c = colours["fog"]
    if d[1] > 0:
        k = 16.0 / d[1]
        px, pz = d[0] * k, d[2] * k
        proj = max(
            px * math.cos(-math.pi + math.pi / 8 + e * math.pi / 4)
            + pz * math.sin(-math.pi + math.pi / 8 + e * math.pi / 4)
            for e in range(8)
        )
        f = proj / (512.0 * math.cos(math.pi / 8))
        if f < 1:
            sph = 16 + (math.hypot(512.0, 16.0) - 16) * f
            cyl = 16 + (512 - 16) * f
            end = min(RENDER_DISTANCE * 16, SKY_FOG_END)
            fv = max(min(max(sph / end, 0.0), 1.0), 0.0 if cyl <= end else 1.0)
            c = [s + (g - s) * fv for s, g in zip(colours["sky"], colours["fog"])]
    if colours["fan"]:
        a = _fan_alpha(colours["fan"], d)
        c = [v * (1 - a) + t * a for v, t in zip(c, colours["fan"][0])]
    return [max(v, 0.0) ** 2.2 for v in c]


def map_direction(i: int, j: int) -> tuple[float, float, float]:
    th = 2 * math.pi * i / MAP_W
    ph = math.pi * j / MAP_H - math.pi / 2
    return (math.cos(ph) * math.cos(th), math.sin(ph), math.cos(ph) * math.sin(th))


def read_pfm(path: Path) -> list[float] | str:
    """The floats of a `PF` map, or why it cannot be read."""
    import struct

    raw = path.read_bytes()
    head = f"PF\n{MAP_W} {MAP_H}\n-1.0\n".encode()
    if not raw.startswith(head):
        return f"{path.name} does not start with the {MAP_W} x {MAP_H} little-endian `PF` header"
    body = raw[len(head) :]
    if len(body) != MAP_W * MAP_H * 12:
        return f"{path.name} holds {len(body)} bytes of pixels, not {MAP_W * MAP_H * 12}"
    return list(struct.unpack(f"<{MAP_W * MAP_H * 3}f", body))


# Where the map is sampled against the gate's own drawing of the game's sky: a
# 24 x 12 grid over the whole sphere, rows from just above the nadir.
SAMPLE_COLUMNS = range(0, MAP_W, 15)
SAMPLE_ROWS = range(7, MAP_H, 15)


def sky_findings(doc: dict, scene_dir: Path | None, weather: str = "clear") -> tuple[list[str], int]:
    """How a scene departs from the game's sky at its tick under its weather,
    if it does, and the tick read. The tick is the scene's `delvewrightSky`
    marker, held to the sun's own altitude by the closed form, and the marker
    names the weather (absent under a clear sky). The map must be the game's
    sky — drawn here, with this gate's own port of the client and its weather
    layer — plus one uniform non-negative veil; and `skyLight` times the map's
    cosine-weighted mean over the upper hemisphere must be the lightmap's
    light."""
    sun = doc.get("sun") or {}
    read = tick_of_sun(sun["altitude"], sun.get("azimuth", 0.0))
    marker = doc.get("delvewrightSky") or {}
    tick = marker.get("daytimeTicks")
    bad: list[str] = []
    if not isinstance(tick, int):
        return [f"carries no `delvewrightSky.daytimeTicks` (its sun reads tick {read})"], read
    if abs((tick - read + 12000) % 24000 - 12000) > TICK_SLACK:
        bad.append(f"names tick {tick}, and its sun stands where tick {read} puts it")
    named = marker.get("weather", "clear")
    if named != weather or (weather == "clear" and "weather" in marker):
        bad.append(f"names the weather {marker.get('weather')!r}; its picture's is {weather!r}")
    sky, fog = doc.get("sky") or {}, doc.get("fog") or {}
    for (obj, key), want in {
        ("sky", "mode"): "SKYMAP_EQUIRECTANGULAR",
        ("sky", "skyMirrored"): False,
        ("sky", "apparentSkyLight"): 1.0,
        ("fog", "uniformDensity"): 0.0,
        ("fog", "skyFogDensity"): 0.0,
        ("sun", "intensity"): 0.0,
    }.items():
        got = (doc.get(obj) or {}).get(key)
        if got != want:
            bad.append(f"`{obj}.{key}` is {got!r}, the game's sky's is {want!r}")
    # The client draws the sun at the alpha 1 - rain level: not at all under weather.
    disc = math.degrees(sun["altitude"]) >= -SUN_DISC_HALF_DEG and weather == "clear"
    if sun.get("drawTexture") != disc:
        bad.append(
            f"`sun.drawTexture` is {sun.get('drawTexture')!r}; the disc is "
            f"{'drawn' if disc else 'not drawn'} at {math.degrees(sun['altitude']):.3f}° under {weather}"
        )
    name = sky.get("skymap")
    if scene_dir is None or not isinstance(name, str):
        return bad + ["names no sky map the gate can read"], tick
    path = scene_dir / name
    if not path.is_file():
        return bad + [f"names the sky map {name}, which was not written beside it"], tick
    px = read_pfm(path)
    if isinstance(px, str):
        return bad + [px], tick
    o = (doc.get("camera") or {}).get("orientation") or {}
    yaw, pitch = o.get("yaw", 0.0), o.get("pitch", 0.0)
    view = (
        math.cos(yaw) * math.sin(pitch),
        -math.cos(pitch),
        -math.sin(yaw) * math.sin(pitch),
    )
    colours = game_sky_colours(tick, view, weather)
    veils = []
    for j in SAMPLE_ROWS:
        for i in SAMPLE_COLUMNS:
            game = game_sky_pixel(colours, map_direction(i, j))
            k = (j * MAP_W + i) * 3
            veils.append([px[k + c] - game[c] for c in range(3)])
    for c in range(3):
        lo, hi = min(v[c] for v in veils), max(v[c] for v in veils)
        if hi - lo > 2e-3 or lo < -1e-3:
            bad.append(
                f"its map is not the game's sky plus one uniform veil: channel {c} departs "
                f"from the gate's drawing by {lo:.5f}..{hi:.5f} over {len(veils)} sample(s)"
            )
    acc, weight = [0.0, 0.0, 0.0], 0.0
    for j in range(MAP_H):
        ph = math.pi * j / MAP_H - math.pi / 2
        dy = math.sin(ph)
        if dy <= 0:
            continue
        w = dy * math.cos(ph)
        row = j * MAP_W * 3
        for c in range(3):
            acc[c] += w * sum(px[row + c : row + MAP_W * 3 : 3])
        weight += w * MAP_W
    light = lightmap_light(tick, weather)
    s = sky.get("skyLight")
    if not isinstance(s, (int, float)):
        bad.append(f"`sky.skyLight` is {s!r}")
    else:
        for c in range(3):
            cast = s * acc[c] / weight
            if abs(cast - light[c]) > 2e-3 * max(light[c], 1e-3):
                bad.append(
                    f"casts {cast:.6f} in channel {c} on an open top face (`skyLight` x the map's "
                    f"mean), where the lightmap's light at tick {tick} under {weather} is {light[c]:.6f}"
                )
    return bad, tick


def sky_matches(
    doc: dict,
    cam: dict,
    design: dict,
    weathers: dict[str, int],
    classes: dict[str, int],
    clear_cells: dict[str, int] | None = None,
    scene_dir: Path | None = None,
) -> list[str]:
    """Whether a camera's scene is under its picture's sky; tallies what it saw."""
    rows = {r["name"]: r for r in design.get("content", {}).get("references", [])}
    row = rows.get(cam["answers"])
    sky = cam.get("sky") or ({"time": row["time"], "weather": row["weather"]} if row else None)
    if sky is None:
        return [f"answers `{cam['answers']}`, which design.json has no row for"]
    bad = judge_sky(doc, sky["weather"], clear_cells, scene_dir)
    alt = math.degrees(doc["sun"]["altitude"])
    cls = "high" if alt >= 20.0 else "low" if alt >= 0.0 else "below"
    weathers[sky["weather"]] = weathers.get(sky["weather"], 0) + 1
    classes[cls] = classes.get(cls, 0) + 1
    return bad


def judge_sky(
    doc: dict,
    weather: str,
    clear_cells: dict[str, int] | None = None,
    scene_dir: Path | None = None,
) -> list[str]:
    """A scene under `weather`: the game's sky of its tick under that weather.
    Tallies scenes by weather and by where on the day their tick falls."""
    bad, tick = sky_findings(doc, scene_dir, weather)
    if clear_cells is not None:
        sky_dark = all(v == 0 for v in timeline_rgb("sky_color", tick))
        light_full = timeline_float("sky_light_factor", tick) == 1.0
        where = "night plateau" if sky_dark else "day plateau" if light_full else "ramp"
        key = f"{weather} {where}"
        clear_cells[key] = clear_cells.get(key, 0) + 1
    return [f"is under {weather} and {why}" for why in bad]


# The tone rule: the scene's curve, evaluated here as the pinned core
# evaluates it, shows the mid-grey anchor at the game's transfer of it.
MID_GREY = 0.18
CORE_EXPONENT = 0.45454543828964233
TONE_TOLERANCE = 1e-4
# The light rule: the core's default emitter intensity (cited) and the lantern
# scale measured in `compiler.md` §4 (authored from that measurement).
CORE_EMITTER_INTENSITY = 13.0
LANTERN_LIGHT_SCALE = 12.2310
REVIEW_GLOW = 0.05 * CORE_EMITTER_INTENSITY
HABLE_KEYS = (
    "shoulderStrength",
    "linearStrength",
    "linearAngle",
    "toeStrength",
    "toeNumerator",
    "toeDenominator",
    "linearWhitePointValue",
)


def _f32(v: float) -> float:
    return struct.unpack("<f", struct.pack("<f", v))[0]


def hable_shown(settings: dict, x: float) -> float:
    """What the pinned core shows for linear `x` at exposure 1 under these
    settings: the products of two parameters and the white scale in float, the
    rest in double, then the core's exponent, clamped at 1."""
    a, b, c, d, e, f, w = (_f32(float(settings[k])) for k in HABLE_KEYS)
    cb, de, df, ef = _f32(c * b), _f32(d * e), _f32(d * f), _f32(e / f)
    num = _f32(_f32(w * _f32(_f32(a * w) + cb)) + de)
    den = _f32(_f32(w * _f32(_f32(a * w) + b)) + df)
    white = _f32(1.0 / _f32(_f32(num / den) - ef))
    y = 2.0 * x
    h = (y * (a * y + cb) + de) / (y * (a * y + b) + df) - ef
    return min(1.0, max(0.0, h * white) ** CORE_EXPONENT)


def tone_findings(doc: dict) -> list[str]:
    """How a scene's tone curve departs from showing mid-grey as the game does."""
    if doc.get("postprocess") != "TONEMAP3":
        return [f"postprocess is {doc.get('postprocess')!r}, not the Hable curve TONEMAP3"]
    settings = doc.get("postprocessSettings")
    if not isinstance(settings, dict) or any(k not in settings for k in HABLE_KEYS):
        return ["carries no complete postprocessSettings: the core would draw its bare preset"]
    shown = hable_shown(settings, MID_GREY)
    game = MID_GREY**CORE_EXPONENT
    bad = []
    if abs(shown - game) > TONE_TOLERANCE:
        bad.append(
            f"shows linear {MID_GREY} at {shown:.6f}, where the game's transfer shows "
            f"{game:.6f}"
        )
    white = float(settings["linearWhitePointValue"])
    if hable_shown(settings, white) < 0.9999:
        bad.append(f"shows its own white point {white} at {hable_shown(settings, white):.6f}")
    want = CORE_EMITTER_INTENSITY * LANTERN_LIGHT_SCALE
    got = doc.get("emitterIntensity")
    if not isinstance(got, (int, float)) or abs(got - want) > 1e-6:
        bad.append(f"emitterIntensity is {got}, where the lantern scale makes it {want:.4f}")
    elif isinstance(doc.get("materials"), dict):
        for name, mat in sorted(doc["materials"].items()):
            glow = float(mat.get("emittance", 0.0)) * got
            if abs(glow - REVIEW_GLOW) > 1e-3:
                bad.append(
                    f"material {name} glows at emittance x intensity {glow:.4f}, where the "
                    f"emulation is calibrated at {REVIEW_GLOW}"
                )
                break
    return bad


MIN_FILL = 1.0 / 3.0


def subject_in_frame(doc: dict, lo: list[int], hi: list[int]) -> tuple[bool, float, float]:
    """Project the layout box through the scene's own camera, as Chunky does.

    Chunky stores `yaw`/`pitch` radians whose world view direction is
    `(cos yaw·sin pitch, -cos pitch, -sin yaw·sin pitch)`; a pixel's ray is
    `forward + x·right + y·up` with `y` in `[-tan(fov/2), tan(fov/2)]` and `x`
    scaled by `width / height`. Returns (every corner inside, width share,
    height share) of the corners' bounding rectangle, clipped to the frame.
    """
    cam = doc["camera"]
    yaw, pitch = cam["orientation"]["yaw"], cam["orientation"]["pitch"]
    f = (math.cos(yaw) * math.sin(pitch), -math.cos(pitch), -math.sin(yaw) * math.sin(pitch))
    n = math.hypot(f[0], f[2])
    right = (-f[2] / n, 0.0, f[0] / n)
    up = (
        right[1] * f[2] - right[2] * f[1],
        right[2] * f[0] - right[0] * f[2],
        right[0] * f[1] - right[1] * f[0],
    )
    pos = (cam["position"]["x"], cam["position"]["y"], cam["position"]["z"])
    tan_v = math.tan(math.radians(cam["fov"]) / 2)
    tan_h = tan_v * doc["width"] / doc["height"]
    xs, ys, inside = [], [], True
    for i in range(8):
        corner = (
            (lo[0], hi[0] + 1)[i & 1],
            (lo[1], hi[1] + 1)[(i >> 1) & 1],
            (lo[2], hi[2] + 1)[(i >> 2) & 1],
        )
        rel = [corner[k] - pos[k] for k in range(3)]
        depth = sum(rel[k] * f[k] for k in range(3))
        if depth <= 0:
            return False, 0.0, 0.0
        x = sum(rel[k] * right[k] for k in range(3)) / depth
        y = sum(rel[k] * up[k] for k in range(3)) / depth
        inside &= abs(x) <= tan_h and abs(y) <= tan_v
        xs.append(x)
        ys.append(y)
    w = (min(max(xs), tan_h) - max(min(xs), -tan_h)) / (2 * tan_h)
    h = (min(max(ys), tan_v) - max(min(ys), -tan_v)) / (2 * tan_v)
    return inside, max(w, 0.0), max(h, 0.0)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--delvec", help="the `delvec` this tool runs. Default: `target/release/delvec` then `target/debug/delvec` in this tree — resolved, NAMED on stderr, and refused when it is older than the compiler sources it was built from (`tools/lib/delvec_bin.py`).")
    ap.add_argument("--prefabs", required=True)
    ap.add_argument("--work", help="scratch dir (default: a temp dir, removed)")
    args = ap.parse_args()

    delvec = resolve_delvec(args.delvec, repo=REPO, caller="check-whole-map-render")

    bases = declared_bases(delvec)
    found = points_by_base()
    unaccounted = [b for b in bases if b not in found]
    if unaccounted:
        die(
            "the engine declares horizon base(s) no gallery point uses, so the "
            "whole-map illustration path is unproven for them:\n"
            + "\n".join(f"  {b}" for b in unaccounted)
            + "\nGive each one a gallery point declaring it, in the change that "
            "added the base."
        )

    tmp = tempfile.TemporaryDirectory() if args.work is None else None
    work = Path(args.work) if args.work else Path(tmp.name)
    work.mkdir(parents=True, exist_ok=True)

    findings: list[str] = []
    scenes_judged = 0
    panoramas_framed = 0
    builds = 0
    cameras_judged = 0
    records = 0
    weathers_seen: dict[str, int] = {}
    classes_seen: dict[str, int] = {}
    clear_seen: dict[str, int] = {}
    plan_skies = 0
    tones_judged = 0

    def engine_runs(base: str) -> dict:
        """Every engine run one base needs, in directories named for its point.

        It judges nothing and says nothing, so the bases run on the shared pool
        (`tools/lib/parallel.py`); the walk below reads them in the declared
        order. `points_by_base` gives each base a distinct point, so no two
        bases share a directory.
        """
        point = found[base]
        label = point or "primary"
        src, out = work / f"src-{label}", work / f"out-{label}"
        materialise(src, None if point is None else GALLERY / "overlays" / point)
        got: dict = {"src": src, "out": out, "label": label}
        got["build"] = run([str(delvec), "build", str(src), "-o", str(out), "--prefabs", args.prefabs])
        if got["build"].returncode != 0:
            return got
        stub_world(out)
        if (src / "design" / "cameras.json").is_file():
            # `delvec cameras` plans and assembles the campaign to stand each
            # camera (spec-0089), so it reads the prefab library the build read.
            got["cameras"] = run(
                [str(delvec), "--prefabs", args.prefabs, "cameras", str(out),
                 "--campaign", str(src), "-o", str(work / f"cameras-{label}")]
            )
        for arm in ("scene", "panorama"):
            cmd = [str(delvec)]
            if arm == "scene":
                cmd += ["--prefabs", args.prefabs]
            cmd += [arm, str(out), "-o", str(work / f"{arm}-{label}")]
            got[arm] = run(cmd)
        return got

    with contextlib.closing(parallel.ordered(engine_runs, bases)) as ran:
        for base, outcome in zip(bases, ran):
            got = outcome.get()
            src, out, label = got["src"], got["out"], got["label"]
            r = got["build"]
            if r.returncode != 0:
                findings.append(
                    f"{base} ({label}): `delvec build` exited {r.returncode} — the point "
                    f"this base is proven on does not compile: {r.stderr.strip().splitlines()[-1:]}"
                )
                continue
            builds += 1
            plan = json.loads((out / "render-plan.json").read_text())
            h = plan.get("horizon")
            kind = h.get("kind") if h else "void"
            if kind != base:
                findings.append(
                    f"{base} ({label}): the build emitted horizon kind `{kind}` — the "
                    "point does not exercise the base it was selected for"
                )
            extent = (h or {}).get("extent")

            record = src / "design" / "cameras.json"
            if record.is_file():
                records += 1
                sheet = json.loads(record.read_text())
                dest = work / f"cameras-{label}"
                r = got["cameras"]
                if r.returncode != 0:
                    findings.append(
                        f"{base} ({label}): `delvec cameras` exited {r.returncode} — "
                        f"{(r.stderr or r.stdout).strip().splitlines()[-1] if (r.stderr or r.stdout).strip() else 'no reason given'}"
                    )
                elif not sheet.get("cameras"):
                    findings.append(f"{base} ({label}): {record} states no camera")
                else:
                    for cam in sheet["cameras"]:
                        scene_file = dest / f"{sheet['campaign_id']}_camera_{cam['name']}.json"
                        if not scene_file.is_file():
                            findings.append(
                                f"{base} ({label}): camera `{cam['name']}` has no scene {scene_file.name}"
                            )
                            continue
                        cameras_judged += 1
                        doc = json.loads(scene_file.read_text())
                        for why in camera_matches(doc, cam):
                            findings.append(f"{base} ({label}) camera `{cam['name']}`: {why}")
                        tones_judged += 1
                        for why in tone_findings(doc):
                            findings.append(f"{base} ({label}) camera `{cam['name']}`: {why}")
                        design = json.loads((src / "design.json").read_text())
                        for why in sky_matches(
                            doc,
                            cam,
                            design,
                            weathers_seen,
                            classes_seen,
                            clear_seen,
                            scene_file.parent,
                        ):
                            findings.append(f"{base} ({label}) camera `{cam['name']}`: {why}")

            for arm in ("scene", "panorama"):
                dest = work / f"{arm}-{label}"
                r = got[arm]
                if r.returncode != 0:
                    findings.append(
                        f"{base} ({label}): `delvec {arm}` exited {r.returncode} — "
                        f"{(r.stderr or r.stdout).strip().splitlines()[-1] if (r.stderr or r.stdout).strip() else 'no reason given'}"
                    )
                    continue
                files = sorted(dest.glob("*.json"))
                if not files:
                    findings.append(
                        f"{base} ({label}): `delvec {arm}` exited 0 and emitted no scene "
                        "file. Exit 0 is not a picture."
                    )
                    continue
                for f in files:
                    scenes_judged += 1
                    doc = json.loads(f.read_text())
                    tones_judged += 1
                    for why in tone_findings(doc):
                        findings.append(f"{base} ({label}) {f.name}: {why}")
                    plan_weather = (plan.get("sky") or {}).get("weather")
                    if plan_weather is None:
                        findings.append(f"{base} ({label}): render-plan.json states no sky.weather")
                    else:
                        plan_skies += 1
                        for why in judge_sky(doc, plan_weather, clear_seen, f.parent):
                            findings.append(f"{base} ({label}) {f.name}: {why}")
                    chunks = doc.get("chunkList") or []
                    if not chunks:
                        findings.append(f"{base} ({label}) {f.name}: empty chunkList")
                        continue
                    if arm == "panorama":
                        panoramas_framed += 1
                        box = plan["layout_aabb"]
                        inside, w, h = subject_in_frame(doc, box["min"], box["max"])
                        if not inside or w * h < MIN_FILL:
                            findings.append(
                                f"{base} ({label}) {f.name}: the placed areas "
                                f"{box['min']}..{box['max']} are "
                                f"{'inside' if inside else 'NOT inside'} the frame and "
                                f"cover {w * h:.2f} of it ({w:.2f} x {h:.2f}); the floor "
                                f"is {MIN_FILL:.2f}. A panorama frames the built place, "
                                "not the ground around it."
                            )
                    if extent:
                        gap = covers(chunks, extent["min"], extent["max"])
                        if gap:
                            findings.append(
                                f"{base} ({label}) {f.name}: the scene loads none of "
                                f"{len(gap)} chunk column(s) of the landform the horizon "
                                f"declares (first: {', '.join(gap[:4])}). The frame would "
                                "show the map standing in a void it is surrounded by "
                                "ground in."
                            )

    print(
        f"camera skies: weathers {dict(sorted(weathers_seen.items()))}, sun bands "
        f"{dict(sorted(classes_seen.items()))}, over {cameras_judged} camera scene(s) and "
        f"{plan_skies} panorama/review scene(s); {sum(clear_seen.values())} scene(s) "
        f"judged against the game's sky under their weather, drawn here from the timeline "
        f"and the weather layer "
        f"({', '.join(f'{k} {v}' for k, v in sorted(clear_seen.items())) or 'none'})."
    )
    print(
        f"tone: {tones_judged} of {cameras_judged + scenes_judged} scene(s) judged for "
        f"mid-grey {MID_GREY} shown at the game's {MID_GREY**CORE_EXPONENT:.6f} "
        f"(tolerance {TONE_TOLERANCE}) and emitterIntensity "
        f"{CORE_EMITTER_INTENSITY * LANTERN_LIGHT_SCALE:.4f}."
    )
    if builds and tones_judged != cameras_judged + scenes_judged:
        findings.append(
            f"the tone rule judged {tones_judged} of {cameras_judged + scenes_judged} scene(s)"
        )
    if builds and not clear_seen:
        findings.append(
            "ZERO scenes were judged against the game's sky: the domain reaches no sky, so the "
            "sky rule examined nothing"
        )
    if cameras_judged and (
        set(weathers_seen) != {"clear", "rain", "thunder"}
        or set(classes_seen) != {"high", "low", "below"}
    ):
        findings.append(
            f"the domain's camera scenes reach weathers {sorted(weathers_seen)} and "
            f"sun bands {sorted(classes_seen)}; the gallery owes every weather and every "
            "sun band a frame (spec-0079 §7)"
        )
    print(
        f"whole-map render: {len(bases)} horizon base(s) declared "
        f"({', '.join(bases)}), {len(found)} point(s) found, {builds} build(s) "
        f"exercised, {scenes_judged} scene file(s) judged, {panoramas_framed} of "
        f"{builds} panorama(s) framed, {cameras_judged} stated camera(s) judged over "
        f"{records} camera record(s), {len(findings)} finding(s)."
    )
    if builds and cameras_judged == 0:
        die(
            "ZERO stated cameras were judged: no point of the domain carries a "
            "`design/cameras.json`, or none of its cameras reached a scene. The "
            "showcase camera path is proven on the gallery's record or not at all."
        )
    if builds and panoramas_framed != builds:
        die(
            f"{panoramas_framed} panorama frame(s) judged over {builds} build(s): "
            "every build owes one, so the framing assertion did not bind to them all."
        )
    if scenes_judged == 0:
        die(
            "ZERO scene files were judged, so every assertion above examined "
            "nothing. A gate that rendered nothing is vacuous, not a pass."
        )
    if findings:
        print("\nfindings:", file=sys.stderr)
        for f in findings:
            print(f"  {f}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
