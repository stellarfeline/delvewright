"""Which coverage regions are production source, and the export they come from.

The one implementation of spec-0126 §1.2's filter, shared by every reader of a
coverage export: `tools/ci/coverage-floor.py` (the floor), `tools/lib/coverage_map.py`
(the per-test map `tools/ci/test-coverage-map.sh` writes) and
`tools/ci/check-suite-audit.py` (the audit table). The production rule is
`crates/delvec/tests/common/source_scan.rs`'s; this copy is held equal to it by
the agreement test `source_scan_agrees`, which asks `coverage-floor.py
--production` and `--production-sources`.

A file's production text is the file with every item a column-zero
`#[cfg(test)]` applies to removed; an out-of-line `#[cfg(test)] mod name;`
takes its file and every file under its directory with it.
"""

from __future__ import annotations

import json
import pathlib

ATTR = "#[cfg(test)]\n"


# --- the production rule (held equal to source_scan.rs) ---------------------


def _is_ident(c: str) -> bool:
    return c.isalnum() or c == "_"


def item_len(s: str) -> int:
    """The length of the item at the start of `s` (source_scan.rs `item_len`)."""
    b = s
    n = len(b)

    def at(k: int) -> str | None:
        return b[k] if 0 <= k < n else None

    def end_of(k: int) -> int:
        return k + 1 if k < n else n

    depth = 0
    k = 0
    while k < n:
        c = b[k]
        if c == "/" and at(k + 1) == "/":
            while k < n and b[k] != "\n":
                k += 1
        elif c == "/" and at(k + 1) == "*":
            nest = 1
            k += 2
            while k < n and nest > 0:
                if b[k] == "/" and at(k + 1) == "*":
                    nest += 1
                    k += 1
                elif b[k] == "*" and at(k + 1) == "/":
                    nest -= 1
                    k += 1
                k += 1
            continue
        elif c == '"':
            k += 1
            while k < n and b[k] != '"':
                if b[k] == "\\":
                    k += 1
                k += 1
        elif (
            c == "r"
            and (
                k == 0
                or not _is_ident(b[k - 1])
                or (b[k - 1] == "b" and (k < 2 or not _is_ident(b[k - 2])))
            )
            and at(k + 1) in ('"', "#")
        ):
            j = k + 1
            hashes = 0
            while at(j) == "#":
                hashes += 1
                j += 1
            if at(j) == '"':
                j += 1
                while j < n:
                    if b[j] == '"' and all(at(j + h) == "#" for h in range(1, hashes + 1)):
                        j += hashes
                        break
                    j += 1
                k = j
        elif c == "'":
            if at(k + 1) == "\\":
                k += 2
                while k < n and b[k] != "'":
                    k += 1
            elif at(k + 2) == "'":
                k += 2
        elif c in "([{":
            depth += 1
        elif c in ")]":
            depth -= 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                j = k + 1
                while at(j) in (" ", "\t"):
                    j += 1
                return end_of(j) if at(j) == ";" else end_of(k)
        elif c == ";" and depth == 0:
            return end_of(k)
        k += 1
    return n


def unscoped(line: str) -> str:
    t = line.lstrip()
    if t.startswith("pub"):
        r = t[3:]
        if r.startswith("("):
            i = r.find(")")
            return (r[i + 1 :] if i >= 0 else r).lstrip()
        if r[:1].isspace():
            return r.lstrip()
    return t


def out_of_line_module(item: str) -> str | None:
    line = None
    for raw in item.splitlines():
        t = raw.strip()
        if t and not t.startswith("//") and not t.startswith("#["):
            line = t
            break
    if line is None or not line.endswith(";"):
        return None
    decl = unscoped(line[:-1])
    if not decl.startswith("mod "):
        return None
    name = decl[4:].strip()
    return name if all(_is_ident(c) for c in name) else None


def production(text: str) -> tuple[str, list[str], list[tuple[int, int]]]:
    """(production text, out-of-line test modules, removed spans as char ranges)."""
    out: list[str] = []
    modules: list[str] = []
    spans: list[tuple[int, int]] = []
    pos = 0
    while True:
        rest = text[pos:]
        if rest.startswith(ATTR):
            at = 0
        else:
            i = rest.find("\n" + ATTR)
            at = i + 1 if i >= 0 else None
        if at is None:
            out.append(rest)
            break
        out.append(rest[:at])
        item = rest[at + len(ATTR) :]
        length = item_len(item)
        name = out_of_line_module(item[:length])
        if name is not None:
            modules.append(name)
        start = pos + at
        end = pos + at + len(ATTR) + length
        spans.append((start, end))
        pos = end
    return "".join(out), modules, spans


def production_sources(src: pathlib.Path) -> list[tuple[str, str]]:
    """Every production `.rs` file under `src` (source_scan.rs `production_sources`)."""
    # Ordered by path component, as a Rust `PathBuf` sorts: `inner/kept.rs`
    # before `inner.rs`.
    files = sorted((p for p in src.rglob("*.rs") if p.is_file()), key=lambda q: q.relative_to(src).parts)
    texts: list[tuple[str, str]] = []
    test_modules: list[str] = []
    for p in files:
        prod, modules, _ = production(p.read_text(encoding="utf-8"))
        r = p.relative_to(src).as_posix()
        stem = r[: -len(".rs")]
        if "/" in stem:
            parent, last = stem.rsplit("/", 1)
            dir_ = parent if last == "mod" else stem
        else:
            dir_ = "" if stem in ("mod", "lib", "main") else stem
        for m in modules:
            test_modules.append(m if not dir_ else f"{dir_}/{m}")
        texts.append((r, prod))

    def kept(r: str) -> bool:
        stem = r[: -len(".rs")]
        if stem.endswith("/mod"):
            stem = stem[: -len("/mod")]
        return not any(stem == m or stem.startswith(m + "/") for m in test_modules)

    return [(r, t) for r, t in texts if kept(r)]


# --- which regions of the export are production ----------------------------


def _src_root(path: str) -> tuple[str, str] | None:
    """(`crates/X/src`, the path relative to it) for a crate source file."""
    parts = path.split("/")
    for i, part in enumerate(parts):
        if part == "src" and i >= 1:
            return "/".join(parts[: i + 1]), "/".join(parts[i + 1 :])
    return None


class Filter:
    """Answers "is this region production?" for the files of one tree."""

    def __init__(self, root: pathlib.Path):
        self.root = root
        self._spans: dict[str, list[tuple[int, int, int, int]] | None] = {}
        self._test_files: dict[str, set[str]] = {}

    def _test_files_under(self, src: str) -> set[str]:
        if src not in self._test_files:
            d = self.root / src
            every = {p.relative_to(d).as_posix() for p in d.rglob("*.rs")} if d.is_dir() else set()
            kept = {r for r, _ in production_sources(d)} if d.is_dir() else set()
            self._test_files[src] = every - kept
        return self._test_files[src]

    def spans(self, path: str) -> list[tuple[int, int, int, int]] | None:
        """The (line, byte column) ranges of `path`'s cfg(test) items, 1-based,
        end exclusive; `None` when the whole file is test code."""
        if path in self._spans:
            return self._spans[path]
        result: list[tuple[int, int, int, int]] | None = []
        sr = _src_root(path)
        if sr is not None and sr[1] in self._test_files_under(sr[0]):
            result = None
        else:
            f = self.root / path
            text = f.read_text(encoding="utf-8") if f.is_file() else ""
            _, _, char_spans = production(text)
            if char_spans:
                starts = [0]
                for i, c in enumerate(text):
                    if c == "\n":
                        starts.append(i + 1)

                def pos(ci: int) -> tuple[int, int]:
                    lo, hi = 0, len(starts) - 1
                    while lo < hi:
                        mid = (lo + hi + 1) // 2
                        if starts[mid] <= ci:
                            lo = mid
                        else:
                            hi = mid - 1
                    col = len(text[starts[lo] : ci].encode("utf-8")) + 1
                    return lo + 1, col

                for a, b in char_spans:
                    result.append((*pos(a), *pos(b)))
        self._spans[path] = result
        return result

    def is_test(self, path: str, line: int, col: int) -> bool:
        spans = self.spans(path)
        if spans is None:
            return True
        return any((l0, c0) <= (line, col) < (l1, c1) for l0, c0, l1, c1 in spans)


# --- reading the export ------------------------------------------------------


def _line_stats(segments: list[list]) -> tuple[int, int]:
    """(lines mapped, lines covered) by LLVM's own line rule over `segments`."""
    by_line: dict[int, list[list]] = {}
    for s in segments:
        by_line.setdefault(s[0], []).append(s)
    if not segments:
        return 0, 0
    mapped = covered = 0
    wrapped = None
    last = max(s[0] for s in segments)
    first = min(s[0] for s in segments)
    for line in range(first, last + 1):
        segs = by_line.get(line, [])
        starts = [s for s in segs if s[3] and s[4] and not s[5]]
        skipped = bool(segs) and not segs[0][3] and segs[0][4] and not starts
        is_mapped = not skipped and ((wrapped is not None and wrapped[3]) or bool(starts))
        if is_mapped:
            count = max(
                ([wrapped[2]] if wrapped is not None and wrapped[3] else [])
                + [s[2] for s in starts]
            )
            mapped += 1
            covered += 1 if count > 0 else 0
        if segs:
            wrapped = segs[-1]
    return mapped, covered


def read_report(report: pathlib.Path, filt: Filter, prefix: str = "") -> dict:
    """The production view of one export: rows, region set, and what was dropped.

    `prefix` is stripped from every file name first: an export written without
    `--remap-path-prefix` names files by absolute path."""
    data = json.loads(report.read_text(encoding="utf-8"))
    return read_export(data["data"][0], filt, prefix)


def read_export(export: dict, filt: Filter, prefix: str = "") -> dict:
    """`read_report` over an already-parsed export object."""
    rows: dict[str, dict] = {}
    regions: dict[tuple[str, int, int], bool] = {}
    excl_regions = 0
    excl_files: set[str] = set()
    skipped: list[str] = []
    for f in export.get("files", []):
        path = f["filename"].replace("\\", "/")
        if prefix and path.startswith(prefix):
            path = path[len(prefix) :]
        if path.startswith("/") or not (filt.root / path).is_file():
            skipped.append(path)
            continue
        kept_segments = []
        r_cov = r_cnt = 0
        for s in f.get("segments", []):
            line, col, count, has_count, entry, gap = s[:6]
            if filt.is_test(path, line, col):
                if has_count and entry and not gap:
                    excl_regions += 1
                    excl_files.add(path)
                continue
            kept_segments.append([line, col, count, has_count, entry, gap])
            if has_count and entry and not gap:
                r_cnt += 1
                key = (path, line, col)
                was = regions.get(key, False)
                regions[key] = was or count > 0
                if count > 0:
                    r_cov += 1
        l_cnt, l_cov = _line_stats(kept_segments)
        rows[path] = {
            "path": path,
            "regions": {"covered": r_cov, "count": r_cnt},
            "lines": {"covered": l_cov, "count": l_cnt},
            "functions": {"covered": 0, "count": 0},
        }
    # Functions: one per (file, start), covered when any instantiation ran.
    funcs: dict[tuple[str, int, int], bool] = {}
    for fn in export.get("functions", []):
        if not fn.get("regions") or not fn.get("filenames"):
            continue
        first = fn["regions"][0]
        path = fn["filenames"][first[5]].replace("\\", "/")
        if prefix and path.startswith(prefix):
            path = path[len(prefix) :]
        if path not in rows:
            continue
        if filt.is_test(path, first[0], first[1]):
            continue
        key = (path, first[0], first[1])
        funcs[key] = funcs.get(key, False) or fn.get("count", 0) > 0
    for (path, _, _), ran in funcs.items():
        rows[path]["functions"]["count"] += 1
        rows[path]["functions"]["covered"] += 1 if ran else 0
    files = [rows[p] for p in sorted(rows)]
    return {
        "files": files,
        "regions": regions,
        "excluded": {"files": len(excl_files), "regions": excl_regions},
        "skipped": sorted(skipped),
    }
