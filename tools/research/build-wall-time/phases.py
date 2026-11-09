#!/usr/bin/env python3
"""Research instrumentation (build wall time): the phase table of one measured run.

Reads a `delvec build` stderr written with `DELVEC_PHASE_TIMING=1` — every
note prefixed `[t=+<wall>s cpu=<cpu>s thr=<n>]` — and prints, per stamped
line, the wall and CPU spent since the previous stamp and the CPU/wall ratio
(the cores busy on average over that interval). With `--phases`, it folds the
intervals into phases: an interval is charged to the LAST `phase:` mark (or
the last note, with `--fine`) that preceded it, and the fold is printed from
the longest down, summing to the run's stamped wall.

    phases.py <stderr> [--phases] [--fine] [--min-seconds N]
"""
import re
import sys

STAMP = re.compile(r"^\[t=\+([0-9.]+)s(?: cpu=([0-9.]+)s thr=(\d+))?\] (.*)$")


def rows(path):
    out = []
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            m = STAMP.match(line.rstrip("\n"))
            if not m:
                continue
            t = float(m.group(1))
            cpu = float(m.group(2)) if m.group(2) else None
            thr = int(m.group(3)) if m.group(3) else None
            out.append((t, cpu, thr, m.group(4)))
    return out


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    path = argv[1]
    fold = "--phases" in argv
    fine = "--fine" in argv
    min_s = 0.0
    if "--min-seconds" in argv:
        min_s = float(argv[argv.index("--min-seconds") + 1])
    rs = rows(path)
    if not rs:
        print("no stamped lines", file=sys.stderr)
        return 1
    prev_t, prev_cpu = 0.0, 0.0
    phase = "(before the first mark)"
    charged = {}
    order = []
    print(f"{'wall@':>8} {'Δwall':>8} {'Δcpu':>8} {'cores':>6} {'thr':>4}  line")
    for t, cpu, thr, text in rs:
        dw = t - prev_t
        dc = (cpu - prev_cpu) if cpu is not None else float("nan")
        cores = dc / dw if dw > 0 else float("nan")
        if dw >= min_s:
            print(f"{t:8.1f} {dw:8.1f} {dc:8.1f} {cores:6.1f} {thr if thr is not None else '':>4}  {text[:150]}")
        key = phase
        if key not in charged:
            charged[key] = [0.0, 0.0]
            order.append(key)
        charged[key][0] += dw
        charged[key][1] += dc if dc == dc else 0.0
        if text.startswith("phase: ") or fine:
            phase = text[:150]
        prev_t = t
        if cpu is not None:
            prev_cpu = cpu
    if fold:
        total_w = sum(v[0] for v in charged.values())
        total_c = sum(v[1] for v in charged.values())
        print()
        print(f"phases charged to the preceding mark; stamped wall {total_w:.1f} s, cpu {total_c:.1f} s")
        print(f"{'wall s':>8} {'%':>5} {'cpu s':>8} {'cores':>6}  phase")
        for k in sorted(charged, key=lambda k: -charged[k][0]):
            w, c = charged[k]
            if w < min_s:
                continue
            print(f"{w:8.1f} {100*w/total_w:5.1f} {c:8.1f} {c/w if w else 0:6.1f}  {k}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
