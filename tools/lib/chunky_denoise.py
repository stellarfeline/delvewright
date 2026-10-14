"""The denoise step of the render route: the pinned denoiser plugin and Open Image
Denoise, installed into the Chunky home, and the passes, the denoise and the
write-back `validation/chunky.sh` runs after every `-render`.

## Why a frame is denoised

The engine scales the core's emitter light so a lantern-lit surface reads, on
average, as the game draws it (`compiler.md` §4, `scene::LANTERN_LIGHT_SCALE`),
and that scale was fitted on denoised frames. At the published sample counts the
scaled emitters leave grain, so every `-render` is followed by this step.

## What the step does

1. **Raw buffer.** A copy of the scene with `outputMode PFM`, snapshotted by the
   pinned core: the sample buffer, linear, before any post-process.
2. **Albedo and normal passes.** A copy of the scene with `renderer
   DenoiserPasses`, rendered by the pinned core with the denoiser plugin loaded
   from a derived Chunky home (`<home>/denoise/home`, which links the home's own
   `lib/`, `versions/` and `resources/` and adds the plugin). The plugin's
   `ObservableValue` update thread keeps that JVM alive after the job is done,
   so the step waits — bounded, `AWAIT_SECONDS` — for the core's "Render job
   finished." line and both maps, then ends the JVM.
3. **Denoise.** `oidnDenoise --hdr raw --alb albedo --nrm normal` (RT filter).
4. **Write-back.** The scene's raw dump is kept as `<scene>.dump.raw` and the
   denoised buffer is written as `<scene>.dump` in the core's uncompressed dump
   format, with the raw dump's sample count and render time, so the core's own
   `-snapshot` applies the scene's post-process: one implementation of the tone
   curve. A later `-render` of the same scene first puts `<scene>.dump.raw` back,
   so samples always accumulate on the raw buffer.

## What is pinned, and where it refuses

`versions.toml [render]`: the plugin jar by URL and sha256, and Open Image Denoise
by version with one archive and one sha256 per platform its project ships. A
platform it ships no archive for builds it from the pinned source release
(`aarch64-unknown-linux-gnu`): the pinned oneTBB source first, then Open Image
Denoise's CPU device with the RT filter's weights and its apps, with the pinned
CMake, Ninja and ISPC for that platform — the machine supplies only a C and C++
compiler (`build_from_source`; `DELVEWRIGHT_BUILD_JOBS` caps the jobs, default
every core). The build records what it was made from (`built-from.json`) and a
later verify holds that record to the pins. A platform with neither an archive
nor a pinned source build, or a source build with no compiler, is refused by
name with what is missing (exit 4); a download whose sha256 differs installs
nothing; a home whose installed files no longer answer the pins is refused before
the render starts. Denoising is never skipped silently.

    python3 tools/lib/chunky_denoise.py platform
    python3 tools/lib/chunky_denoise.py install --home <chunky home> [--work DIR]
    python3 tools/lib/chunky_denoise.py verify --home <chunky home>
    python3 tools/lib/chunky_denoise.py oidn --home <chunky home>
    python3 tools/lib/chunky_denoise.py denoise-home --home <chunky home>
    python3 tools/lib/chunky_denoise.py restore --scene-dir D --scene N
    python3 tools/lib/chunky_denoise.py prepare --scene-dir D --scene N --work W
    python3 tools/lib/chunky_denoise.py await-maps --dir W/aux --scene N --log L --pid P
    python3 tools/lib/chunky_denoise.py writeback --pfm F --scene-dir D --scene N

Exit 0 on success; 3 when the home does not hold the pinned kit (named, with the
remedy); 4 when the platform has no pinned archive; 2 when a pin cannot be read;
1 for any other failure. Stdlib only.
"""

from __future__ import annotations

import argparse
import dataclasses
import hashlib
import subprocess
import json
import os
import pathlib
import platform as _platform
import shutil
import signal
import struct
import sys
import tarfile
import time
import urllib.request
import zipfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import versions  # noqa: E402

INSTALLER = "validation/chunky-install.sh"
SENTINEL = "Render job finished."
AWAIT_SECONDS = 900
PASSES_SPP = 16


class KitError(RuntimeError):
    def __init__(self, msg: str, code: int = 3):
        super().__init__(msg)
        self.code = code


def render_pins() -> dict:
    try:
        return versions.load()["render"]
    except (OSError, KeyError) as e:
        raise KitError(f"cannot read versions.toml [render]: {e}", 2)


def pin(pins: dict, key: str) -> str:
    v = pins.get(key)
    if not isinstance(v, str) or not v:
        raise KitError(f"versions.toml [render] has no `{key}`", 2)
    return v


def host_triple() -> str:
    system = _platform.system()
    machine = _platform.machine().lower()
    arch = {"arm64": "aarch64", "aarch64": "aarch64", "x86_64": "x86_64", "amd64": "x86_64"}.get(machine, machine)
    if system == "Darwin":
        return f"{arch}-apple-darwin"
    if system == "Linux":
        return f"{arch}-unknown-linux-gnu"
    if system == "Windows":
        return f"{arch}-pc-windows-msvc"
    return f"{arch}-{system.lower()}"


@dataclasses.dataclass
class Plan:
    """How Open Image Denoise reaches this platform: the project's own archive, or a
    build from its pinned source release with pinned build tools."""

    kind: str  # "archive" or "source"
    name: str  # the archive kept under <kit>/archives (prebuilt or source)
    sha: str
    url: str
    tools: dict  # source builds: tool -> (file name, sha256, url)


BUILD_TOOLS = ("cmake", "ninja", "ispc")


def oidn_plan(pins: dict, triple: str) -> Plan:
    key = triple.replace("-", "_")
    name, sha = pins.get(f"oidn_archive_{key}"), pins.get(f"oidn_sha256_{key}")
    if name and sha:
        return Plan("archive", name, sha, pin(pins, "oidn_url_base") + name, {})
    if all(pins.get(f"oidn_build_{t}_{key}") for t in BUILD_TOOLS) and pins.get("oidn_source_archive"):
        tools = {}
        for t in BUILD_TOOLS:
            fname = pin(pins, f"oidn_build_{t}_{key}")
            tools[t] = (fname, pin(pins, f"oidn_build_{t}_sha256_{key}"), pin(pins, f"oidn_build_{t}_url_base") + fname)
        tbb = pin(pins, "oidn_build_tbb_url")
        tools["tbb"] = (pin(pins, "oidn_build_tbb_name"), pin(pins, "oidn_build_tbb_sha256"), tbb)
        src = pin(pins, "oidn_source_archive")
        return Plan("source", src, pin(pins, "oidn_source_sha256"), pin(pins, "oidn_url_base") + src, tools)
    def triple_of(key: str) -> str:
        return key.replace("_", "-").replace("x86-64", "x86_64")

    shipped = sorted(triple_of(k[len("oidn_archive_"):]) for k in pins if k.startswith("oidn_archive_"))
    built = sorted(triple_of(k[len("oidn_build_cmake_"):]) for k in pins
                   if k.startswith("oidn_build_cmake_") and not k.startswith("oidn_build_cmake_sha256_")
                   and not k.startswith("oidn_build_cmake_url"))
    raise KitError(
        f"Open Image Denoise {pins.get('oidn_version', '?')} ships no archive for {triple} and no source build "
        f"is pinned for it (archives: {', '.join(shipped) or 'none'}; source builds: {', '.join(built) or 'none'}), "
        "so a frame rendered here cannot be denoised and validation/chunky.sh refuses to render it.",
        4,
    )


def compilers() -> dict:
    """The C and C++ compilers a source build would use, or the names of the missing."""
    found = {}
    for lang, env, names in (("c", "CC", ("cc", "gcc", "clang")), ("c++", "CXX", ("c++", "g++", "clang++"))):
        cand = [os.environ[env]] if os.environ.get(env) else list(names)
        hit = next((shutil.which(c) for c in cand if shutil.which(c)), None)
        found[lang] = hit
    return found


def require_compilers() -> dict:
    found = compilers()
    missing = [f"a {lang} compiler ({'/'.join(n)} on PATH, or ${e})" for lang, e, n in
               (("c", "CC", ("cc", "gcc", "clang")), ("c++", "CXX", ("c++", "g++", "clang++"))) if not found[lang]]
    if missing:
        raise KitError(
            "Open Image Denoise is built from source on this platform, and the build cannot run here: missing "
            + " and ".join(missing) + ". Install them (e.g. `apt-get install g++`) and run "
            f"{INSTALLER} again; nothing was installed.",
            4,
        )
    return found


def oidn_archive(pins: dict, triple: str) -> tuple[str, str]:
    """The prebuilt archive for `triple`; raises when the platform builds from source or is refused."""
    plan = oidn_plan(pins, triple)
    if plan.kind != "archive":
        raise KitError(f"{triple} builds Open Image Denoise from source; it has no prebuilt archive", 4)
    return plan.name, plan.sha


def sha256(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def kit_dir(home: pathlib.Path) -> pathlib.Path:
    return home / "denoise"


def plugin_name(pins: dict) -> str:
    return pin(pins, "denoiser_plugin_url").rsplit("/", 1)[-1]


def oidn_binary(root: pathlib.Path) -> pathlib.Path:
    for name in ("oidnDenoise", "oidnDenoise.exe"):
        hits = sorted(root.glob(f"*/bin/{name}")) + sorted(root.glob(f"bin/{name}"))
        if hits:
            return hits[0]
    raise KitError(f"no oidnDenoise under {root}", 3)


def built_from(plan: Plan) -> dict:
    """What a source build was made from: the record a later verify compares."""
    return {"source": plan.sha, **{t: v[1] for t, v in sorted(plan.tools.items())}}


def verify(home: pathlib.Path) -> dict:
    """The installed kit, held to the pins; raises KitError naming what is wrong."""
    pins = render_pins()
    triple = host_triple()
    plan = oidn_plan(pins, triple)
    kit = kit_dir(home)
    remedy = f"Run {INSTALLER}."
    jar = kit / "home" / "plugins" / plugin_name(pins)
    if not jar.is_file():
        raise KitError(f"the denoiser plugin is not installed at {jar}. {remedy}")
    if sha256(jar) != pin(pins, "denoiser_plugin_sha256"):
        raise KitError(f"{jar} does not answer versions.toml [render] denoiser_plugin_sha256. {remedy}")
    arc = kit / "archives" / plan.name
    if not arc.is_file():
        raise KitError(f"Open Image Denoise is not installed: {arc} is missing. {remedy}")
    which = "oidn_sha256" if plan.kind == "archive" else "oidn_source_sha256"
    if sha256(arc) != plan.sha:
        raise KitError(f"{arc} does not answer versions.toml [render] {which} for {triple}. {remedy}")
    root = kit / "oidn" / plan.sha
    if plan.kind == "source":
        rec = root / "built-from.json"
        if not rec.is_file() or json.loads(rec.read_text()) != built_from(plan):
            raise KitError(f"the Open Image Denoise build at {root} was not made from the pinned source and tools. {remedy}")
    binary = oidn_binary(root)
    for link in ("lib", "versions", "resources"):
        if not (kit / "home" / link).exists():
            raise KitError(f"the derived Chunky home {kit / 'home'} has no {link}. {remedy}")
    return {"plugin": str(jar), "oidn": str(binary), "home": str(kit / "home"), "triple": triple, "kind": plan.kind}


def fetch(url: str, dest: pathlib.Path, want: str) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    part = dest.with_name(dest.name + ".part")
    if url.startswith("file://"):
        # A local pin (the test suite's stub kit) is read as a file, not fetched.
        shutil.copyfile(urllib.request.url2pathname(url[len("file://"):]), part)
    else:
        with urllib.request.urlopen(url) as r, open(part, "wb") as f:
            shutil.copyfileobj(r, f)
    got = sha256(part)
    if got != want:
        part.unlink()
        raise KitError(f"{url} answered sha256 {got}, where versions.toml [render] pins {want}; nothing was installed", 1)
    part.replace(dest)


def _extract(archive: pathlib.Path, dest: pathlib.Path) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    if archive.name.endswith(".zip"):
        with zipfile.ZipFile(archive) as z:
            for info in z.infolist():
                z.extract(info, dest)
                mode = info.external_attr >> 16
                if mode:
                    os.chmod(dest / info.filename, mode)
    else:
        with tarfile.open(archive) as t:
            t.extractall(dest, filter="tar")


def _find(root: pathlib.Path, name: str) -> pathlib.Path:
    hits = sorted(p for p in root.rglob(name) if p.is_file())
    if not hits:
        raise KitError(f"no {name} under {root}", 1)
    return hits[0]


def build_jobs() -> int:
    return max(1, int(os.environ.get("DELVEWRIGHT_BUILD_JOBS") or os.cpu_count() or 1))


def _run(cmd: list, log: pathlib.Path, **kw) -> None:
    with open(log, "a") as f:
        f.write("$ " + " ".join(str(c) for c in cmd) + "\n")
        f.flush()
        r = subprocess.run([str(c) for c in cmd], stdout=f, stderr=subprocess.STDOUT, **kw)
    if r.returncode != 0:
        tail = log.read_text(errors="replace").splitlines()[-15:]
        raise KitError(f"`{cmd[0]}` exited {r.returncode} while building Open Image Denoise (log {log}):\n  " + "\n  ".join(tail), 1)


def build_from_source(plan: Plan, src_archive: pathlib.Path, work: pathlib.Path, prefix: pathlib.Path) -> None:
    """oneTBB, then Open Image Denoise (CPU device, the RT filter's weights, its apps),
    with the pinned CMake, Ninja and ISPC, into `prefix` (bin/oidnDenoise, lib/ with
    TBB beside it)."""
    found = require_compilers()
    jobs = build_jobs()
    log = work / "build.log"
    t0 = time.monotonic()
    tools = {}
    for t, (fname, sha, url) in plan.tools.items():
        f = work / "dl" / fname
        fetch(url, f, sha)
        tools[t] = f
    for t in ("cmake", "ninja", "ispc"):
        _extract(tools[t], work / "tools" / t)
    cmake = _find(work / "tools" / "cmake", "cmake")
    ninja = _find(work / "tools" / "ninja", "ninja")
    ispc = _find(work / "tools" / "ispc", "ispc")
    for b in (cmake, ninja, ispc):
        b.chmod(b.stat().st_mode | 0o111)
    env = dict(os.environ, CC=found["c"], CXX=found["c++"])
    common = [f"-DCMAKE_MAKE_PROGRAM={ninja}", "-G", "Ninja", "-DCMAKE_BUILD_TYPE=Release"]
    _extract(tools["tbb"], work / "tbb-src")
    tbb_src = next(p for p in (work / "tbb-src").iterdir() if p.is_dir())
    tbb_prefix = work / "tbb"
    _run([cmake, "-S", tbb_src, "-B", work / "tbb-build", *common, f"-DCMAKE_INSTALL_PREFIX={tbb_prefix}",
          "-DTBB_TEST=OFF", "-DTBB_EXAMPLES=OFF", "-DTBB_STRICT=OFF"], log, env=env)
    _run([cmake, "--build", work / "tbb-build", "-j", jobs], log, env=env)
    _run([cmake, "--install", work / "tbb-build"], log, env=env)
    _extract(src_archive, work / "oidn-src")
    oidn_src = next(p for p in (work / "oidn-src").iterdir() if p.is_dir())
    _run([cmake, "-S", oidn_src, "-B", work / "oidn-build", *common, f"-DCMAKE_INSTALL_PREFIX={prefix}",
          f"-DTBB_ROOT={tbb_prefix}", f"-DISPC_EXECUTABLE={ispc}", "-DOIDN_APPS=ON", "-DOIDN_FILTER_RTLIGHTMAP=OFF",
          "-DOIDN_INSTALL_DEPENDENCIES=ON", "-DCMAKE_INSTALL_RPATH=$ORIGIN/../lib"], log, env=env)
    _run([cmake, "--build", work / "oidn-build", "-j", jobs], log, env=env)
    _run([cmake, "--install", work / "oidn-build"], log, env=env)
    for lib in sorted((tbb_prefix).rglob("libtbb*.so*")):
        dest = prefix / "lib" / lib.name
        if not dest.exists():
            shutil.copy2(lib, dest, follow_symlinks=False)
    oidn_binary(prefix.parent)
    print(f"chunky_denoise: built Open Image Denoise from source in {time.monotonic() - t0:.0f} s with {jobs} job(s)",
          file=sys.stderr)


def install(home: pathlib.Path, work: pathlib.Path) -> dict:
    pins = render_pins()
    triple = host_triple()
    plan = oidn_plan(pins, triple)
    try:
        return verify(home)
    except KitError as e:
        if e.code == 4:
            raise
    if plan.kind == "source":
        require_compilers()
    kit = kit_dir(home)
    work.mkdir(parents=True, exist_ok=True)
    # Every download is verified in the work directory before anything enters the home.
    jar_tmp = work / plugin_name(pins)
    arc_tmp = work / plan.name
    root = kit / "oidn" / plan.sha
    try:
        fetch(pin(pins, "denoiser_plugin_url"), jar_tmp, pin(pins, "denoiser_plugin_sha256"))
        fetch(plan.url, arc_tmp, plan.sha)
        if root.exists():
            shutil.rmtree(root)
        if plan.kind == "archive":
            _extract(arc_tmp, root)
        else:
            build_from_source(plan, arc_tmp, work, root / f"oidn-{pins.get('oidn_version', 'source')}-source")
            (root / "built-from.json").write_text(json.dumps(built_from(plan), indent=2, sort_keys=True) + "\n")
    except BaseException:
        shutil.rmtree(work, ignore_errors=True)
        if root.exists():
            shutil.rmtree(root, ignore_errors=True)
        raise
    (kit / "archives").mkdir(parents=True, exist_ok=True)
    shutil.copy2(arc_tmp, kit / "archives" / plan.name)
    dh = kit / "home"
    (dh / "plugins").mkdir(parents=True, exist_ok=True)
    shutil.copy2(jar_tmp, dh / "plugins" / plugin_name(pins))
    for link in ("lib", "versions", "resources"):
        target = home / link
        p = dh / link
        if p.is_symlink() or p.exists():
            continue
        try:
            p.symlink_to(target, target_is_directory=True)
        except OSError:
            shutil.copytree(target, p)
    (dh / "chunky.json").write_text(json.dumps({"plugins": [plugin_name(pins)]}, indent=2) + "\n")
    shutil.rmtree(work)
    return verify(home)


# ---- the per-scene step -------------------------------------------------------

def restore(scene_dir: pathlib.Path, scene: str) -> bool:
    raw = scene_dir / f"{scene}.dump.raw"
    if raw.is_file():
        raw.replace(scene_dir / f"{scene}.dump")
        return True
    return False


def prepare(scene_dir: pathlib.Path, scene: str, work: pathlib.Path) -> None:
    src = json.loads((scene_dir / f"{scene}.json").read_text())
    if work.exists():
        shutil.rmtree(work)
    for sub, extra in (("raw", (".dump",)), ("aux", ())):
        d = work / sub
        d.mkdir(parents=True)
        for ext in (".octree2", ".sky.pfm", *extra):
            f = scene_dir / f"{scene}{ext}"
            if f.is_file():
                shutil.copy2(f, d / f.name)
    raw = dict(src)
    raw["outputMode"] = "PFM"
    (work / "raw" / f"{scene}.json").write_text(json.dumps(raw, indent=2))
    aux = dict(src)
    aux["renderer"] = "DenoiserPasses"
    aux["spp"] = 0
    aux["renderTime"] = 0
    aux["sppTarget"] = PASSES_SPP
    data = dict(aux.get("additionalData") or {})
    data["denoiser"] = {
        "saveBeauty": False, "enableAlbedo": True, "albedoSpp": PASSES_SPP, "saveAlbedo": True,
        "enableNormal": True, "normalSpp": PASSES_SPP, "saveNormal": True, "normalWaterDisplacement": True,
    }
    aux["additionalData"] = data
    (work / "aux" / f"{scene}.json").write_text(json.dumps(aux, indent=2))


def _alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


def await_maps(d: pathlib.Path, scene: str, log: pathlib.Path, pid: int, timeout: int = AWAIT_SECONDS) -> None:
    """Wait for the passes JVM's job to finish (its sentinel line and both maps), then
    end it: the plugin's update thread would keep it alive forever."""
    maps = [d / f"{scene}.albedo.pfm", d / f"{scene}.normal.pfm"]
    deadline = time.monotonic() + timeout
    done = False
    while time.monotonic() < deadline:
        text = log.read_text(errors="replace") if log.is_file() else ""
        if SENTINEL in text and all(m.is_file() and m.stat().st_size > 0 for m in maps):
            done = True
            break
        if not _alive(pid):
            break
        time.sleep(0.5)
    if _alive(pid):
        os.kill(pid, signal.SIGTERM)
        for _ in range(20):
            if not _alive(pid):
                break
            time.sleep(0.5)
        if _alive(pid):
            os.kill(pid, signal.SIGKILL)
    if not done:
        missing = [m.name for m in maps if not (m.is_file() and m.stat().st_size > 0)]
        raise KitError(
            f"the albedo and normal passes did not finish within {timeout} s "
            f"(sentinel {'seen' if SENTINEL in (log.read_text(errors='replace') if log.is_file() else '') else 'not seen'}, "
            f"missing {missing or 'none'}); see {log}",
            1,
        )


def read_pfm(path: pathlib.Path) -> tuple[int, int, bytes]:
    with open(path, "rb") as f:
        if f.readline().strip() != b"PF":
            raise KitError(f"{path} is not a colour PFM", 1)
        w, h = map(int, f.readline().split())
        scale = float(f.readline())
        data = f.read()
    if len(data) != w * h * 12:
        raise KitError(f"{path} holds {len(data)} bytes for {w} x {h}", 1)
    return w, h, data if scale < 0 else _swap32(data)


def _swap32(data: bytes) -> bytes:
    n = len(data) // 4
    return struct.pack(f"<{n}f", *struct.unpack(f">{n}f", data))


def dump_header(path: pathlib.Path) -> tuple[int, int, int, int]:
    """(width, height, spp, renderTime) of a core render dump, any format with a magic."""
    with open(path, "rb") as f:
        head = f.read(32)
    if head[:4] != b"DUMP":
        raise KitError(f"{path} carries no dump magic", 1)
    _, w, h, spp, rt = struct.unpack(">iiiiq", head[4:28])
    return w, h, spp, rt


def writeback(pfm: pathlib.Path, scene_dir: pathlib.Path, scene: str) -> None:
    dump = scene_dir / f"{scene}.dump"
    w, h, spp, rt = dump_header(dump)
    pw, ph, data = read_pfm(pfm)
    if (pw, ph) != (w, h):
        raise KitError(f"{pfm} is {pw} x {ph}, the dump {w} x {h}", 1)
    n = w * h * 3
    floats = struct.unpack(f"<{n}f", data)
    out = bytearray(b"DUMP" + struct.pack(">iiiiq", 2, w, h, spp, rt))
    row = w * 3
    # PFM rows run bottom to top; the dump's run top to bottom.
    for y in range(h - 1, -1, -1):
        out += struct.pack(f">{row}d", *floats[y * row:(y + 1) * row])
    dump.replace(scene_dir / f"{scene}.dump.raw")
    tmp = scene_dir / f"{scene}.dump.part"
    tmp.write_bytes(bytes(out))
    tmp.replace(dump)


def _main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(prog="chunky_denoise.py")
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("platform")
    for name in ("verify", "oidn", "denoise-home"):
        sub.add_parser(name).add_argument("--home", required=True)
    i = sub.add_parser("install")
    i.add_argument("--home", required=True)
    i.add_argument("--work")
    for name in ("restore", "prepare", "writeback", "await-maps"):
        p = sub.add_parser(name)
        if name in ("restore", "prepare", "writeback"):
            p.add_argument("--scene-dir", required=True)
        p.add_argument("--scene", required=True)
    sub.choices["prepare"].add_argument("--work", required=True)
    sub.choices["writeback"].add_argument("--pfm", required=True)
    a = sub.choices["await-maps"]
    a.add_argument("--dir", required=True)
    a.add_argument("--log", required=True)
    a.add_argument("--pid", type=int, required=True)
    args = ap.parse_args(argv)
    try:
        if args.cmd == "platform":
            print(host_triple())
        elif args.cmd == "verify":
            k = verify(pathlib.Path(args.home))
            print(f"denoise kit verified for {k['triple']}: plugin {k['plugin']}, {k['oidn']}")
        elif args.cmd == "oidn":
            print(verify(pathlib.Path(args.home))["oidn"])
        elif args.cmd == "denoise-home":
            print(verify(pathlib.Path(args.home))["home"])
        elif args.cmd == "install":
            home = pathlib.Path(args.home)
            k = install(home, pathlib.Path(args.work) if args.work else kit_dir(home) / "work")
            print(f"denoise kit installed for {k['triple']}: plugin {k['plugin']}, {k['oidn']}")
        elif args.cmd == "restore":
            if restore(pathlib.Path(args.scene_dir), args.scene):
                print(f"restored the raw dump of {args.scene}")
        elif args.cmd == "prepare":
            prepare(pathlib.Path(args.scene_dir), args.scene, pathlib.Path(args.work))
        elif args.cmd == "await-maps":
            await_maps(pathlib.Path(args.dir), args.scene, pathlib.Path(args.log), args.pid)
        elif args.cmd == "writeback":
            writeback(pathlib.Path(args.pfm), pathlib.Path(args.scene_dir), args.scene)
    except KitError as e:
        print(f"chunky_denoise: {e}", file=sys.stderr)
        return e.code
    return 0


if __name__ == "__main__":
    sys.exit(_main(sys.argv[1:]))
