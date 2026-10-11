// Dumps, for every blockstate of a pinned Minecraft server jar, what the game's
// own `BlockState.canSurvive(LevelReader, BlockPos)` needs of the six cells
// beside it — the question the server asks of a wall torch, a lantern, a flower,
// a rail or a door half the first time a shape update reaches it, and drops the
// block when the answer is no.
//
// `canSurvive` is code, not data, so this does not read it: it ASKS it. Each
// state S is set at BlockPos.ZERO in a level that holds nothing but S and air
// (a java.lang.reflect.Proxy over LevelReader whose getBlockState answers from a
// map), and:
//
//   1. in air alone. A state that survives there needs no support: `free`.
//   2. otherwise, once for every neighbour cell d (down, up, north, south, west,
//      east) and every blockstate N of the registry, with N at d and air
//      everywhere else. The set of N for which S survives is S's support from d.
//
// A support set is written as the nearest of the base sets the game itself
// names — `none`, `full`/`center`/`rigid` (N.isFaceSturdy(face toward S,
// SupportType.FULL/CENTER/RIGID)) and `nonair` (!N.isAir()) — plus the states it
// adds to and removes from that base, by registry index; the driver
// (`tools/maintenance/dump-support.py`) turns indices into names and collapses
// them. Nothing is classified by hand: the base is picked by the size of the
// difference, and the difference is written whole.
//
// A state is `unjudged`, with the reason, when the single-neighbour reading
// cannot be the whole of its rule:
//
//   far       canSurvive read a cell that is neither S's own nor a neighbour's
//             (sugar cane's water beside the block under it, scaffolding's
//             distance);
//   none      no single neighbour keeps it (a rule that needs two at once, such
//             as a big dripleaf stem's, or one this level cannot satisfy);
//   error     canSurvive asked the level something it does not model (light,
//             a chunk, a registry) — the message is written;
//   combined  a seeded sample of whole neighbourhoods found S surviving where
//             no single neighbour in that neighbourhood keeps it (step 3).
//
//   3. Cross-check: for every judged state, a fixed number of neighbourhoods
//      with all six cells filled, drawn from a seeded java.util.SplittableRandom
//      (seed and count on the command line) half from S's own support sets and
//      half from the whole registry. Wherever the game says S survives, one of
//      the six cells must be in S's support set from its side; otherwise S is
//      `combined`. A neighbourhood where the game says S does NOT survive while
//      a support is present is counted (a rule with a second, killing
//      condition, like a cactus beside a wall) and reported, not refused: the
//      table answers what holds a block, not everything that can break it.
//
// Also written, per state, the faces the game calls CENTER- and RIGID-sturdy
// (the FULL faces are `faces-<version>.tsv`'s `sturdy` column).
//
// This file hardcodes NO obfuscated name. Every class/member it touches is passed
// in on the command line as key=value, resolved from the official Mojang
// mappings for the same pin, by exact signature, by the driver.
package dw;

import java.io.BufferedWriter;
import java.io.FileWriter;
import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.BitSet;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.SplittableRandom;

public final class SupportDump {
    private SupportDump() {}

    static Map<String, String> arg = new HashMap<>();

    static String a(String k) {
        String v = arg.get(k);
        if (v == null) {
            throw new IllegalStateException("missing argument " + k);
        }
        return v;
    }

    static Class<?> cls(String k) throws Exception {
        return Class.forName(a(k));
    }

    static Object staticField(String clsKey, String fieldKey) throws Exception {
        Field f = cls(clsKey).getDeclaredField(a(fieldKey));
        f.setAccessible(true);
        return f.get(null);
    }

    static Method method(String clsKey, String name, Class<?>... params) throws Exception {
        Method m = cls(clsKey).getDeclaredMethod(a(clsKey + "." + name), params);
        m.setAccessible(true);
        return m;
    }

    /** Thrown by the level for a question it does not model. */
    static final class Unmodelled extends RuntimeException {
        Unmodelled(String what) {
            super(what, null, false, false);
        }
    }

    /** Thrown by the level when canSurvive reads a cell beyond the neighbours. */
    static final class Far extends RuntimeException {
        Far(Object pos) {
            super(String.valueOf(pos), null, false, false);
        }
    }

    // The probe level: S at ZERO, `cells` beside it, air everywhere else.
    static final Map<Object, Object> cells = new HashMap<>();
    static Object self;
    static Object air;
    static Object zero;
    static final java.util.Set<Object> near = new java.util.HashSet<>();
    static Method getFluidStateOfState;

    public static void main(String[] argv) throws Exception {
        for (int i = 2; i < argv.length; i++) {
            String[] kv = argv[i].split("=", 2);
            arg.put(kv[0], kv[1]);
        }
        String out = argv[0];
        String faceOut = argv[1];

        method("SharedConstants", "tryDetectVersion").invoke(null);
        method("Bootstrap", "bootStrap").invoke(null);

        // Bind the vanilla block and fluid tags, as a server's reload does: a
        // flower's soil, a sapling's dirt, a coral's water are tag questions,
        // and an unbound tag answers no to every block.
        Class<?> packType = cls("PackType");
        Object serverData = staticField("PackType", "PackType.SERVER_DATA");
        Object vanilla = method("ServerPacksSource", "createVanillaPackSource").invoke(null);
        Class<?> resourceManagerCls = cls("ResourceManager");
        Object rm = cls("MultiPackResourceManager").getDeclaredConstructor(packType, List.class)
                .newInstance(serverData, List.of(vanilla));
        Class<?> registryCls = cls("Registry");
        Method loadPending = method("TagLoader", "loadPendingTags", resourceManagerCls, registryCls);
        Method apply = method("PendingTags", "apply");
        int boundRegistries = 0;
        for (String reg : new String[] {"BuiltInRegistries.BLOCK", "BuiltInRegistries.FLUID"}) {
            Object registry = staticField("BuiltInRegistries", reg);
            java.util.Optional<?> pending = (java.util.Optional<?>) loadPending.invoke(null, rm, registry);
            if (pending.isEmpty()) {
                throw new IllegalStateException("no tags loaded for " + reg);
            }
            apply.invoke(pending.get());
            boundRegistries++;
        }

        Class<?> direction = cls("Direction");
        Class<?> blockPos = cls("BlockPos");
        Class<?> blockGetter = cls("BlockGetter");
        Class<?> levelReader = cls("LevelReader");
        Class<?> supportType = cls("SupportType");
        Class<?> stateBase = cls("BlockStateBase");
        Object stateRegistry = staticField("Block", "Block.BLOCK_STATE_REGISTRY");
        Method canSurvive = method("BlockStateBase", "canSurvive", levelReader, blockPos);
        Method isAir = method("BlockStateBase", "isAir");
        Method isSolid = method("BlockStateBase", "isSolid");
        Method fluidType = method("FluidState", "getType");
        Object water = staticField("Fluids", "Fluids.WATER");
        getFluidStateOfState = method("BlockStateBase", "getFluidState");
        Method sturdy4 = method("BlockStateBase", "isFaceSturdy", blockGetter, blockPos, direction, supportType);
        Method relative = method("BlockPos", "relative", direction);
        Object empty = staticField("EmptyBlockGetter", "EmptyBlockGetter.INSTANCE");
        zero = staticField("BlockPos", "BlockPos.ZERO");
        String[] dirKeys = {"DOWN", "UP", "NORTH", "SOUTH", "WEST", "EAST"};
        int[] opposite = {1, 0, 3, 2, 5, 4};
        Object[] dirs = new Object[6];
        Object[] at = new Object[6];
        for (int i = 0; i < 6; i++) {
            dirs[i] = staticField("Direction", "Direction." + dirKeys[i]);
            at[i] = relative.invoke(zero, dirs[i]);
            near.add(at[i]);
        }
        near.add(zero);
        Object[] support = {
            staticField("SupportType", "SupportType.FULL"),
            staticField("SupportType", "SupportType.CENTER"),
            staticField("SupportType", "SupportType.RIGID"),
        };

        List<Object> all = new ArrayList<>();
        for (Object st : (Iterable<?>) stateRegistry) {
            all.add(st);
            if (st.toString().equals("Block{minecraft:air}")) {
                air = st;
            }
        }
        if (air == null) {
            throw new IllegalStateException("no minecraft:air in the registry");
        }
        int n = all.size();

        // The level's own questions, matched by obfuscated name and parameter
        // count: the getters it answers, the height it states, the rest refused.
        String getBlockState = a("BlockGetter.getBlockState");
        String getFluidState = a("BlockGetter.getFluidState");
        String getBlockEntity = a("BlockGetter.getBlockEntity");
        String getMinY = a("LevelHeightAccessor.getMinY");
        String getHeight = a("LevelHeightAccessor.getHeight");
        String isClientSide = a("LevelReader.isClientSide");
        int minY = Integer.parseInt(a("minY"));
        int height = Integer.parseInt(a("height"));
        InvocationHandler h = (proxy, m, args) -> {
            String name = m.getName();
            int pc = m.getParameterCount();
            if (pc == 1 && blockPos.isInstance(args[0])) {
                if (name.equals(getBlockState)) {
                    return stateAt(args[0]);
                }
                if (name.equals(getFluidState)) {
                    return getFluidStateOfState.invoke(stateAt(args[0]));
                }
                if (name.equals(getBlockEntity)) {
                    return null;
                }
            }
            if (pc == 0 && name.equals(getMinY) && m.getReturnType() == int.class) {
                return minY;
            }
            if (pc == 0 && name.equals(getHeight) && m.getReturnType() == int.class) {
                return height;
            }
            if (pc == 0 && name.equals(isClientSide) && m.getReturnType() == boolean.class) {
                return false;
            }
            if (m.getDeclaringClass() == Object.class) {
                switch (name) {
                    case "hashCode": return System.identityHashCode(proxy);
                    case "equals": return proxy == args[0];
                    case "toString": return "dw.SupportDump.level";
                    default: break;
                }
            }
            if (m.isDefault()) {
                return InvocationHandler.invokeDefault(proxy, m, args);
            }
            throw new Unmodelled(m.getDeclaringClass().getName() + "." + name + "/" + pc);
        };
        Object level = Proxy.newProxyInstance(SupportDump.class.getClassLoader(), new Class<?>[] {levelReader}, h);

        // The base sets, over registry index: per face of N (the face toward S),
        // FULL / CENTER / RIGID sturdy; and non-air.
        BitSet[][] sturdySet = new BitSet[3][6];
        for (int t = 0; t < 3; t++) {
            for (int f = 0; f < 6; f++) {
                sturdySet[t][f] = new BitSet(n);
            }
        }
        BitSet nonAir = new BitSet(n);
        BitSet solid = new BitSet(n);
        BitSet waterSet = new BitSet(n);
        PrintWriter faces = new PrintWriter(new BufferedWriter(new FileWriter(faceOut)));
        String letters = "dunswe";
        for (int i = 0; i < n; i++) {
            Object st = all.get(i);
            if (!(Boolean) isAir.invoke(st)) {
                nonAir.set(i);
            }
            StringBuilder flags = new StringBuilder();
            if ((Boolean) isAir.invoke(st)) {
                flags.append('a');
            }
            if ((Boolean) isSolid.invoke(st)) {
                solid.set(i);
                flags.append('s');
            }
            if (fluidType.invoke(getFluidStateOfState.invoke(st)) == water) {
                waterSet.set(i);
                flags.append('w');
            }
            StringBuilder[] col = {new StringBuilder(), new StringBuilder(), new StringBuilder()};
            for (int t = 0; t < 3; t++) {
                for (int f = 0; f < 6; f++) {
                    if ((Boolean) sturdy4.invoke(st, empty, zero, dirs[f], support[t])) {
                        sturdySet[t][f].set(i);
                        col[t].append(letters.charAt(f));
                    }
                }
            }
            faces.println(st + "\t" + dash(col[1]) + "\t" + dash(col[2]) + "\t" + dash(flags));
        }
        faces.close();

        long seed = Long.parseLong(a("seed"));
        int samples = Integer.parseInt(a("samples"));

        PrintWriter w = new PrintWriter(new BufferedWriter(new FileWriter(out)));
        int free = 0;
        int supported = 0;
        Map<String, Integer> unjudged = new LinkedHashMap<>();
        long probes = 0;
        long killedWithSupport = 0;
        for (int si = 0; si < n; si++) {
            if (si % 2000 == 0) {
                System.err.println("  state " + si + "/" + n + " probes " + probes);
            }
            Object s = all.get(si);
            self = s;
            cells.clear();
            String verdict;
            try {
                probes++;
                if ((Boolean) unwrap(canSurvive, s, level, zero)) {
                    w.println(s + "\tfree");
                    free++;
                    continue;
                }
                BitSet[] sets = new BitSet[6];
                boolean any = false;
                for (int d = 0; d < 6; d++) {
                    BitSet set = new BitSet(n);
                    for (int ni = 0; ni < n; ni++) {
                        cells.clear();
                        cells.put(at[d], all.get(ni));
                        probes++;
                        if ((Boolean) unwrap(canSurvive, s, level, zero)) {
                            set.set(ni);
                        }
                    }
                    sets[d] = set;
                    any |= !set.isEmpty();
                }
                if (!any) {
                    verdict = "unjudged\tnone";
                } else {
                    // Step 3: whole neighbourhoods, seeded per state.
                    SplittableRandom rnd = new SplittableRandom(seed ^ (si * 0x9E3779B97F4A7C15L));
                    List<int[]> pools = new ArrayList<>();
                    for (int d = 0; d < 6; d++) {
                        pools.add(sets[d].stream().toArray());
                    }
                    String combined = null;
                    for (int k = 0; k < samples && combined == null; k++) {
                        cells.clear();
                        int[] pick = new int[6];
                        for (int d = 0; d < 6; d++) {
                            int[] pool = pools.get(rnd.nextInt(6));
                            pick[d] = (rnd.nextBoolean() && pool.length > 0)
                                    ? pool[rnd.nextInt(pool.length)]
                                    : rnd.nextInt(n);
                            cells.put(at[d], all.get(pick[d]));
                        }
                        probes++;
                        boolean lives = (Boolean) unwrap(canSurvive, s, level, zero);
                        boolean held = false;
                        for (int d = 0; d < 6; d++) {
                            held |= sets[d].get(pick[d]);
                        }
                        if (lives && !held) {
                            StringBuilder b = new StringBuilder("combined");
                            for (int d = 0; d < 6; d++) {
                                b.append(d == 0 ? " " : ",").append(letters.charAt(d)).append('=').append(all.get(pick[d]));
                            }
                            combined = b.toString();
                        } else if (!lives && held) {
                            killedWithSupport++;
                        }
                    }
                    if (combined != null) {
                        verdict = "unjudged\t" + combined;
                    } else {
                        StringBuilder b = new StringBuilder("support");
                        for (int d = 0; d < 6; d++) {
                            b.append('\t').append(letters.charAt(d)).append(':')
                                    .append(encode(sets[d], sturdySet, opposite[d], nonAir, solid, waterSet));
                        }
                        verdict = b.toString();
                        supported++;
                    }
                }
            } catch (Far e) {
                verdict = "unjudged\tfar " + e.getMessage();
            } catch (Unmodelled e) {
                verdict = "unjudged\terror " + e.getMessage();
            }
            String kind = verdict.split("\t", 3)[0].equals("unjudged") ? verdict.split("\t")[1].split(" ")[0] : null;
            if (kind != null) {
                unjudged.merge(kind, 1, Integer::sum);
            }
            w.println(s + "\t" + verdict);
        }
        w.close();
        int unj = unjudged.values().stream().mapToInt(Integer::intValue).sum();
        System.out.println("DUMPED states=" + n + " free=" + free + " supported=" + supported
                + " unjudged=" + unj + " tag-registries=" + boundRegistries + " probes=" + probes
                + " killed-with-support=" + killedWithSupport);
        System.out.println("UNJUDGED " + unjudged);
    }

    static String dash(StringBuilder b) {
        return b.length() == 0 ? "-" : b.toString();
    }

    static Object stateAt(Object pos) {
        if (!near.contains(pos)) {
            throw new Far(pos);
        }
        if (pos.equals(zero)) {
            return self;
        }
        Object st = cells.get(pos);
        return st == null ? air : st;
    }

    static Object unwrap(Method m, Object recv, Object... args) throws Exception {
        try {
            return m.invoke(recv, args);
        } catch (InvocationTargetException e) {
            Throwable c = e.getCause();
            while (c instanceof InvocationTargetException || c instanceof java.lang.reflect.UndeclaredThrowableException) {
                c = c.getCause();
            }
            if (c instanceof Far) {
                throw (Far) c;
            }
            if (c instanceof Unmodelled) {
                throw (Unmodelled) c;
            }
            throw new Unmodelled(c.getClass().getName() + ": " + c.getMessage());
        }
    }

    /**
     * A support set as `base+i,j-k,l`: the base of the fewest differences among
     * none, full, center, rigid (sturdy on face `f` of N, the face toward S) and
     * nonair, ties broken in that order.
     */
    static String encode(BitSet set, BitSet[][] sturdySet, int f, BitSet nonAir, BitSet solid, BitSet water) {
        String[] names = {"none", "full", "center", "rigid", "nonair", "solid", "water"};
        BitSet[] bases = {new BitSet(), sturdySet[0][f], sturdySet[1][f], sturdySet[2][f], nonAir, solid, water};
        int best = -1;
        int bestCost = Integer.MAX_VALUE;
        for (int b = 0; b < bases.length; b++) {
            BitSet x = (BitSet) set.clone();
            x.xor(bases[b]);
            if (x.cardinality() < bestCost) {
                bestCost = x.cardinality();
                best = b;
            }
        }
        BitSet plus = (BitSet) set.clone();
        plus.andNot(bases[best]);
        BitSet minus = (BitSet) bases[best].clone();
        minus.andNot(set);
        StringBuilder b = new StringBuilder(names[best]);
        if (!plus.isEmpty()) {
            b.append('+').append(join(plus));
        }
        if (!minus.isEmpty()) {
            b.append('-').append(join(minus));
        }
        return b.toString();
    }

    static String join(BitSet s) {
        StringBuilder b = new StringBuilder();
        for (int i = s.nextSetBit(0); i >= 0; i = s.nextSetBit(i + 1)) {
            if (b.length() > 0) {
                b.append(',');
            }
            b.append(i);
        }
        return b.toString();
    }
}
