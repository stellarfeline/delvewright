# Can a static-musl `delvec` reach a Linux GPU? — ADR-0021 §3, verified

ADR-0021 §3 keeps `piece`/`batch`/`fidelity-gate` out of the distributed binary
partly on a claim its author marked **unverified**:

> the shelf's Linux targets are musl-static on purpose (no glibc floor), and a
> fully static musl binary cannot `dlopen` a Vulkan loader, which is how wgpu
> reaches a Linux GPU

## Verdict

**CONFIRMED, on a real Linux host, end to end.** A fully static musl binary
cannot `dlopen` anything at all — musl's static libc answers
`Dynamic loading not supported` before touching the filesystem — and the real
`delve-render` GPU path built for `aarch64-unknown-linux-musl` therefore
reports `DW0723 gpu init: NoGpuAdapter` against a Vulkan stack that a
glibc build of the *same source, same container, same driver* renders on
successfully.

Two further, independent blockers were found that ADR-0021 does not name, and
that would stop a folded-in wgpu **earlier** than `dlopen` does — see
"Beyond the claim".

## Host

OrbStack on macOS/arm64. Debian trixie `linux/arm64` container (`muslvk`) and
Alpine 3.21 `linux/arm64` container (`alpvk`), plus one throwaway
`linux/amd64` Debian for the x86_64 shelf target. Software Vulkan only — no
GPU is needed to answer the question:

```
$ vulkaninfo --summary
Vulkan Instance Version: 1.4.309
deviceName = llvmpipe (LLVM 19.1.7, 128 bits)   driverID = DRIVER_ID_MESA_LLVMPIPE
```

Toolchain: rustup `1.97.1` (the repo's pinned channel), targets
`aarch64-unknown-linux-musl`, `x86_64-unknown-linux-musl`,
`aarch64-unknown-linux-gnu`.

## Q3 first — is static musl actually what the shelf builds?

Yes. `versions.toml [engine].targets` lists `x86_64-unknown-linux-musl` and
`aarch64-unknown-linux-musl`, and `tools/build-release-binaries.sh` asserts
every musl artifact carries **no `PT_INTERP`**. Reproduced on the probe
binaries below with that script's own ELF reader:

```
PT_INTERP present: False | program headers: 9
```

So the ADR's premise is a fact of the release recipe, not an assumption.

## Q1 — can a static-musl Rust binary `dlopen` a shared library?

No. A 40-line probe with **no crates** (`dlopen`/`dlsym`/`dlerror` declared
directly, so nothing but libc is in play), built with the shelf's exact
`RUSTFLAGS` (`-C strip=symbols -C linker=<sysroot>/…/rust-lld
-C linker-flavor=ld.lld`):

```
$ file dltest              # aarch64 static musl
ELF 64-bit LSB executable, ARM aarch64, statically linked, stripped
$ ./dltest
dlopen(libvulkan.so.1) = NULL   error: Dynamic loading not supported         # exit 1

$ ./dltest                 # aarch64-unknown-linux-gnu control, same source
dlopen(libvulkan.so.1) = 0xaaaadbc58e00  OK
dlsym(vkGetInstanceProcAddr) = 0xffffa84d2c20  OK                            # exit 0
```

Same result for the other shelf target, run under `linux/amd64`:

```
$ file dltest-x86-staticmusl
ELF 64-bit LSB pie executable, x86-64, static-pie linked, stripped
$ /work/dltest-x86-staticmusl                       # on x86_64 Debian
dlopen(libvulkan.so.1) = NULL   error: Dynamic loading not supported         # exit 1
```

Reproduced natively on Alpine with a musl-host rustup toolchain (no
cross-compilation anywhere in the loop), which also isolates the cause to
`crt-static` rather than to musl:

```
# static (default for the musl target)
statically linked → dlopen(libvulkan.so.1) = NULL   Dynamic loading not supported
# RUSTFLAGS="-C target-feature=-crt-static"
interpreter /lib/ld-musl-aarch64.so.1 → dlopen OK, dlsym OK
```

## Q2 — can the real render crate enumerate ANY adapter when built for musl?

The real thing, not a toy: `crates/render` with only its dependency line
changed to ADR-0021 §2's proposal, `nucleation = { version = "=0.10.8",
features = ["rendering"] }`. Resolves to `nucleation 0.10.8` + `wgpu 30.0.0`,
and `grep -c "git+" Cargo.lock` → **0** (ADR-0021 §2's claim, incidentally
reproduced). Textures are the owner's own 1.21.11 client jar, sha1
`ba2df812c2d12e0219c489c4cd9a5e1f0760f5bd` — equal to the value ADR-0021 §5
pins from piston-meta.

Same command, same container, same lavapipe, same jar; only the target differs.

```
# A. static musl (aarch64-unknown-linux-musl, no PT_INTERP, stripped, 10,715,096 B)
$ delve-render --textures /work/minecraft.jar --size 256 fidelity-gate --out /tmp/m.png
DW0723 [error] gpu init: NoGpuAdapter
exit=5

# B. glibc control (aarch64-unknown-linux-gnu), byte-for-byte the same source
$ delve-render --textures /work/minecraft.jar --size 256 fidelity-gate --out /tmp/g.png
fidelity gate PASSED: no missing-texture placeholder in the newest-block fixture
exit=0   → fidelity-gate.png, 20,931 B
```

`strace -e trace=openat` shows the mechanism directly — the static binary never
reaches the filesystem at all, because `dlopen` fails first:

```
vulkan/EGL/GL objects opened by the static musl binary : 0
vulkan/EGL/GL objects opened by the glibc control      : 164
  openat("/lib/aarch64-linux-gnu/libvulkan.so.1", O_RDONLY|O_CLOEXEC) = 3
  openat("/etc/vulkan/implicit_layer.d", …) = 3
```

`wgpu → gpu-allocator → wgpu-hal → ash → libloading` is the chain
(`cargo tree -i libloading`), and `libloading` is `dlopen`. The ADR's stated
mechanism is exactly right.

And the falsifying case, for completeness — the same crate built as a
**dynamically linked musl** binary on Alpine:

```
$ file delve-render
ELF 64-bit LSB pie executable, ARM aarch64, interpreter /lib/ld-musl-aarch64.so.1, stripped
$ delve-render --textures /work/minecraft.jar --size 256 fidelity-gate --out /tmp/dyn.png
fidelity gate PASSED: no missing-texture placeholder in the newest-block fixture
exit=0   → 113 vulkan/icd opens
```

That render is **byte-identical** to the glibc one
(`sha256 e98ba4bca9fe333f84146ab9cd2e162d89ec313262cf2a7f453ba5b992526380`),
across two distros and two LLVM builds of lavapipe.

So the barrier is `crt-static`, and dropping it does make Linux GPU rendering
work. What it costs is the property the shelf exists for: that binary does not
start on a machine without musl's loader.

```
# the dynamic-musl binary, on the glibc Debian container
Error loading shared library libgcc_s.so.1: No such file or directory
Error relocating …: _Unwind_SetIP: symbol not found
```

On a distro with no musl at all it fails earlier still, at the missing
`/lib/ld-musl-aarch64.so.1` interpreter. Whether trading that away is
acceptable is a decision for the owner, not a finding.

## Beyond the claim — two blockers ADR-0021 does not name

Both were hit while trying to *build* the wgpu stack for musl, i.e. before
`dlopen` is ever reached, and both are stronger than the claim §3 rests on.

**1. `nucleation` needs a musl C cross-compiler, and the shelf is built without
one.** `blake3 1.8.6` is a **non-optional** dependency of `nucleation 0.10.8`
(it is not behind `rendering` — a bare `nucleation = "=0.10.8"` pulls it), and
its build script compiles C:

```
$ cargo check --release --target aarch64-unknown-linux-musl     # no musl gcc on PATH
error: failed to run custom build command for `blake3 v1.8.6`
  error occurred in cc-rs: failed to find tool "aarch64-linux-musl-gcc": No such file or directory
exit=101
```

This contradicts the standing premise recorded beside the target list —
"`delvec`'s whole dependency set is pure Rust … so a static binary has no glibc
floor" — and the reason `tools/build-release-binaries.sh` uses `rust-lld`:
"no apt step on the runner and NOTHING on a macOS workstation". `cargo check`
runs build scripts, so the `engine binaries (cross-build shelf)` gate is
exactly where this lands: it would go red on both musl targets the moment
`delvec` acquired a `nucleation` dependency of any kind.

**2. The shelf's linker cannot resolve `-ldl`.** `libloading` emits `-ldl`;
rustc's self-contained musl sysroot has no `libdl.a` (musl folds `dl` into
libc), so the shelf's exact recipe cannot link the wgpu stack at all:

```
rust-lld: error: unable to find library -ldl
```

The static binary in section Q2 exists only because Debian's `musl-dev` was
raided for its **empty** `libdl.a` (`ar t libdl.a` → 0 members) and fed in with
`-L native=…`. Nothing in the release recipe supplies that today.

## Was anything else in §3 wrong?

No — the rest of §3 and its supporting Context item check out on this tree:

- **"the only arms reaching `nucleation`/`wgpu` are `piece`, `batch`,
  `fidelity-gate`"** — verified at module level, not just by `grep`.
  `render.rs` is used only by `main.rs`, and only from those three arms;
  `nbt::build_schematic` (the one nucleation-typed function in `nbt.rs`) is
  called only from `render.rs`. The CPU arms `scene`, `panorama`,
  `contact-sheet`, `index` reach `scene`/`panorama`/`sheet`/`index` only —
  none of which touch `nbt` or `nucleation`. §1's move of the CPU surface into
  `delvec` therefore does not drag `nucleation` (and so does not trigger
  blocker 1 above).
- **`nucleation` without `rendering` does not pull `wgpu`/`ash`/`naga`** —
  confirmed by `cargo tree`; only `blake3` comes along regardless.

## What this does NOT settle

- Only `aarch64` was exercised end to end with the real crate. `x86_64` static
  musl was verified with the `dlopen` probe under emulation. The mechanism is
  in libc, not in the architecture, but the full render binary was not built
  for `x86_64-unknown-linux-musl`.
- No real GPU was involved anywhere; lavapipe is a software ICD. That is
  deliberate — the question is whether the loader can be reached at all — but
  it means nothing here speaks to driver-specific behaviour.
- Whether a `-crt-static` or `gnu` Linux shelf target is an acceptable trade is
  a product decision, untouched by this note.
