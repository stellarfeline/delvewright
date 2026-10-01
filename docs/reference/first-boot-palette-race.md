# "Missing Palette entry" at first boot — what the record holds

Reader: an engineer looking at a delve server that crashed with `MissingPaletteEntryException` ("Getting block state / Missing Palette entry for index N") on a `Worker-Main` thread, shortly after `Done`, before any player joined. Each claim says how it was established. The instrument for every number below is the released image `ghcr.io/stellarfeline/delve-vesperhold:v1.0.0` (OCI index `sha256:5ef9453f2f02544257a089cf2b6f5dd013c09d129a66075533221000a0f8dfca`; `/delve/manifest.json` sha256 `49e1d71abd3d5cbb894416c29a6639663984b02d10ad77c1c043b9730f658127`) and the pinned server jar (`versions.toml` `[minecraft]`, sha256 `f83b8e09…1726`), deobfuscated with Mojang's official `server_mappings` for 1.21.11 (sha1 `5621e9253f05fd57872bbe7f8ddf5f9a7d525955`, from the version JSON the piston-meta manifest lists for 1.21.11).

## 1. Which palette: a chunk section's, never a structure template's

**Measured (bytecode + mappings).** The frames resolve as:

| obfuscated | official name |
|---|---|
| `eqv` | `net.minecraft.world.level.chunk.MissingPaletteEntryException` |
| `equ.a(SourceFile:70)` | `LinearPalette.valueFor(int)` (lines 67–70) |
| `eqy.a(SourceFile:151)` | `PalettedContainer.get(int)` (lines 150–151) |
| `cbq.run(SourceFile:62)` | `AbstractConsecutiveExecutor.run()` (lines 62–67) |

The crash-report title "Getting block state" is written by exactly one class, `LevelChunk` (`eqq`), in the `catch` of `getBlockState(BlockPos)`. Only three classes construct `MissingPaletteEntryException`: `LinearPalette`, `HashMapPalette`, `GlobalPalette` — the chunk-section palette family behind `PalettedContainer`. A structure template's palette is `StructureTemplate$SimplePalette` (`fjq$c`), whose `stateFor(int)` returns `Blocks.AIR`'s default state for an id it does not hold; it cannot raise this exception. A `LinearPalette` serves a section holding at most 16 distinct states, so "index 9" names the tenth distinct state of one 16×16×16 section.

**Measured (shipped bytes).** Every template the image ships is in range anyway: 254 of 254 `.nbt` files opened, 15,973,904 block entries checked, 0 with a `state` outside its palette (largest palette 68, every `DataVersion` 4671). The checker reds on one planted out-of-range `state`.

## 2. Which code path: the light worker reading a section the main thread is writing

**Measured (thread dumps of a fresh arm64 boot).** Twenty `SIGQUIT` dumps, one every 2 s from `Done`, while the `#minecraft:load` bootstrap places the campaign's templates (the tick that logs `Can't keep up! … Running ≈20000ms`). In 7 of the 20 dumps the `Server thread` is inside the placement; in 3 of those 7 it is in `ServerFunctionManager → PlaceCommand → StructureTemplate.placeInWorld → Level.setBlock` while a `Worker-Main` thread is in:

```
LightEngine.propagateDecreases / runLightUpdates
LevelLightEngine.runLightUpdates
ThreadedLevelLightEngine.runUpdate            (line 184)
ThreadedLevelLightEngine lambda$addTask        (line 108)
ChunkTaskDispatcher                            (line 88)
AbstractConsecutiveExecutor.run                (line 62)   ← the issue's bottom frame
```

`LightEngine.getState(BlockPos)` calls `LightChunk.getBlockState`, which on a loaded chunk is `LevelChunk.getBlockState` → `LevelChunkSection.getBlockState` → `PalettedContainer.get` → `LinearPalette.valueFor` — the issue's top frames. `ChunkMap` builds that executor as `new ConsecutiveExecutor(worker pool, "light")`; a second one, `"worldgen"`, has the same bottom frame. The frames the issue elides (between `eqy.a` and `cbq.run`) decide between them; the light path is the one observed running concurrently with the placement.

**Why it can throw (bytecode).** `LinearPalette.idFor` appends with plain stores — `values[size] = state; size = size + 1` — and `PalettedContainer.set` then stores the new index into the bit storage, also plainly. `PalettedContainer`'s `ThreadingDetector` guards writers only; `get` takes no lock and `size` is not volatile. A reader on another thread that loads the storage cell (9) and then `size` (still 9) has no happens-before edge with the writer, so the Java memory model permits exactly `Missing Palette entry for index 9`.

**Measured (probe against the pinned jar's own classes).** `Race.java` below writes 16 distinct states into fresh `PalettedContainer`s on one thread, as `LevelChunkSection.setBlockState` does, while a second thread reads them, as the light worker does.

| platform | runs | reads | `MissingPaletteEntryException` |
|---|---|---|---|
| linux/arm64, native (Apple silicon) | 4 (20 s + 3 × 60 s) | 43,799,049,444 | 62,912 |
| linux/amd64 under Rosetta (x86 store/load ordering) | 4 (4 × 60 s) | 37,054,976,460 | 0 |

The indices thrown vary run to run (2, 5, 7, 13 first). On arm64 the hardware may make the storage store visible before the `size` store; x86 ordering does not. The race is vanilla's and is architecture-dependent: an arm64 host is exposed, an x86 host practically is not.

## 3. Reproduction by boot

**Measured.** Fresh boots of the released image on this workstation (10 cores, Docker VM 12 GiB + swap), "fresh" meaning what the image's own reset loop does: `/data/world` and `/data/usercache.json` removed, the jar and libraries kept. Each boot is watched until the server logs `Server empty for 60 seconds, pausing` (placement finished, then 60 s idle) or the container exits.

| config | platform | boots | crashes | 95% upper bound on per-boot rate |
|---|---|---|---|---|
| `MEMORY=4G` | arm64 | 40 | 0 | 7.2% |
| `MEMORY=2G`, `--cpus=4` | arm64 | 30 | 0 | 9.5% |
| `MEMORY=4G`, `--cpus=4`, `--memory=2560m` (≈1.6 GiB of the heap in swap, sampled from the cgroup) | arm64 | 20 | 0 | 13.9% |
| `MEMORY=4G` | amd64 under Rosetta | 10 | 0 | 25.9% |

Pooled arm64: 0 crashes in 90 boots bounds the per-boot rate below 3.3% at 95% on this hardware. Every boot logged zero `ERROR` lines. The placement tick ran 15.3–40.0 s behind on arm64 and 29.2–44.5 s under Rosetta.

**The defect is unreproduced by boot.** It is reproduced as a mechanism (§2): the pinned jar's own section palette throws this exception under a concurrent unlocked read on arm64 and not under x86 ordering. A host that is slower than this workstation (fewer cores, swapping) lengthens the placement tick, which is the window in which the light worker reads sections the main thread is filling; whether memory pressure raises the rate is not established by these boots.

## 4. What it means for the engine

Nothing the compiler emits is implicated: the throwing palette is the server's own, built from the states the server itself writes, and the templates' palettes are in range and of a class that cannot throw it. The engine's `#minecraft:load` bootstrap is the trigger exposure only, because it is a long burst of main-thread block writes while the light engine works; every vanilla arm64 server performing block writes near lit chunks is exposed the same way. There is no compiler fix.

**Upstream.** No Mojira report names this path. The open report of the same exception on a `Worker-Main` thread is [MC-274474](https://mojira.dev/MC-274474) ("Chunks stopped loading / missing palette entry for index 4", 1.21, Plausible; the moderator could not reproduce it and called it possibly hardware-specific; the reporter's platform is not stated). An upstream report naming the light-worker read and the arm64 evidence above is not yet filed.

## Probe source

Compile and run in `eclipse-temurin:21-jdk` against the pinned server jar with its `META-INF/*.SF`/`*.RSA` signature files removed (the probe sits in the unnamed package beside the obfuscated classes) and the bundler's `libraries/` on the classpath: `java Race <seconds> <distinct-states>`.

```java
// eqy = PalettedContainer, erd = Strategy, dzq.k = Block.BLOCK_STATE_REGISTRY,
// jj = IdMapper (a = byId), eqv = MissingPaletteEntryException,
// w.a = SharedConstants.tryDetectVersion, amv.a = Bootstrap.bootStrap
import java.util.concurrent.atomic.*;

public class Race {
    static volatile eqy current;
    static volatile boolean stop;

    @SuppressWarnings({"rawtypes", "unchecked"})
    public static void main(String[] args) throws Exception {
        int seconds = Integer.parseInt(args[0]);
        int distinct = Integer.parseInt(args[1]);
        w.a();
        amv.a();
        jj reg = dzq.k;
        Object air = reg.a(0);
        Object[] states = new Object[distinct];
        for (int i = 0; i < distinct; i++) states[i] = reg.a(1 + i * 17);
        erd strat = erd.a(reg);
        AtomicLong reads = new AtomicLong(), missing = new AtomicLong(), other = new AtomicLong(), containers = new AtomicLong();
        AtomicReference<String> firstMsg = new AtomicReference<>();
        Thread reader = new Thread(() -> {
            long r = 0;
            while (!stop) {
                eqy c = current;
                if (c == null) continue;
                for (int i = 0; i < distinct; i++) {
                    try {
                        c.a(i, 0, 0);
                    } catch (eqv e) {
                        missing.incrementAndGet();
                        firstMsg.compareAndSet(null, e.getMessage());
                    } catch (Throwable t) {
                        other.incrementAndGet();
                        firstMsg.compareAndSet(null, t.toString());
                    }
                    r++;
                }
            }
            reads.set(r);
        }, "reader");
        reader.start();
        long end = System.nanoTime() + seconds * 1_000_000_000L;
        while (System.nanoTime() < end) {
            eqy c = new eqy(air, strat);
            current = c;
            for (int i = 0; i < distinct; i++) c.c(i, 0, 0, states[i]);
            containers.incrementAndGet();
        }
        stop = true;
        reader.join();
        System.out.printf("arch=%s seconds=%d distinct=%d containers=%d reads=%d missing_palette=%d other=%d first=%s%n",
                System.getProperty("os.arch"), seconds, distinct, containers.get(), reads.get(), missing.get(), other.get(), firstMsg.get());
    }
}
```
