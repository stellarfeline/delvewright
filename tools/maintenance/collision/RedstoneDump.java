// Dumps, for every blockstate of a pinned Minecraft server jar, the two redstone
// facts the game answers per state — and, per block, the classes whose bytecode
// decides the third — for `tools/maintenance/dump-redstone.py` (spec-0100 §4.2).
//
//   conductor   `BlockState.isRedstoneConductor(BlockGetter, BlockPos)`: whether a
//               strong signal handed to this cell powers its neighbours.
//   shape-full  `BlockState.isCollisionShapeFullBlock(BlockGetter, BlockPos)`: the
//               game's default for `conductor`, so the driver can say whether a
//               block conducts `always`, `never`, or by its `shape`.
//   faces-full  whether `Block.isFaceFull(getCollisionShape(...), face)` holds for
//               all six faces — the reading the engine has of "full block" (the
//               face table); the driver asserts it equals `shape-full` on every
//               state, so the engine's reading of `shape` is the game's.
//   source      `BlockState.isSignalSource()`.
//
// Mode `assignable` answers, for each class name in a file, whether it is
// assignable to a given class (the receiver test of the `reads_signal` column).
//
// This file hardcodes NO obfuscated name. Every class/member it touches is passed
// in on the command line, resolved from the official Mojang mappings for the same
// pin by `tools/maintenance/dump-redstone.py`.
//
// argv (mode `states`):
//   0  "states"
//   1  out: one line per blockstate, "<BlockState.toString()>\t<conductor>\t<shape-full>\t<faces-full>\t<source>"
//   2  out: one line per block, "<a BlockState.toString() of it>\t<block class chain>\t<block-entity class chain>"
//      the block-entity chain is "-" for a block that is no EntityBlock and "!" for one whose
//      newBlockEntity(BlockPos.ZERO, state) returns null
//      each chain comma-separated, most derived first, stopping before the stop class
//   3  SharedConstants class            4  .tryDetectVersion()
//   5  Bootstrap class                  6  .bootStrap()
//   7  Block class                      8  .BLOCK_STATE_REGISTRY   9  .isFaceFull(VoxelShape, Direction)
//  10  BlockBehaviour$BlockStateBase   11  .getCollisionShape(BlockGetter, BlockPos)
//  12  .isRedstoneConductor(BlockGetter, BlockPos)   13  .isCollisionShapeFullBlock(BlockGetter, BlockPos)
//  14  .isSignalSource()               15  .getBlock()
//  16  EmptyBlockGetter class          17  .INSTANCE
//  18  BlockPos class                  19  .ZERO
//  20  Direction class                 21..26  .DOWN .UP .NORTH .SOUTH .WEST .EAST
//  27  BlockBehaviour class (the block chain's stop)
//  28  EntityBlock interface           29  .newBlockEntity(BlockPos, BlockState)
//  30  BlockEntity class (the block-entity chain's stop)
//
// argv (mode `assignable`):
//   0  "assignable"   1  the class every name is tested against
//   2  in: one class name per line     3  out: "<name>\t<true|false>" per line
package dw;

import java.io.BufferedWriter;
import java.io.FileWriter;
import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.LinkedHashMap;
import java.util.Map;

public final class RedstoneDump {
    private RedstoneDump() {}

    private static Method staticNoArg(String cls, String name) throws Exception {
        Method m = Class.forName(cls).getDeclaredMethod(name);
        m.setAccessible(true);
        return m;
    }

    private static Object staticField(String cls, String name) throws Exception {
        Field f = Class.forName(cls).getDeclaredField(name);
        f.setAccessible(true);
        return f.get(null);
    }

    /**
     * The one declared method of `cls` called `name` taking `arity` arguments
     * whose LAST parameter is `last` (or any, when null). An ambiguity is refused
     * rather than resolved by declaration order.
     */
    private static Method byArity(String cls, String name, int arity, Class<?> last) throws Exception {
        Method found = null;
        for (Method m : Class.forName(cls).getDeclaredMethods()) {
            if (m.getName().equals(name) && m.getParameterCount() == arity
                    && (arity == 0 || last == null || m.getParameterTypes()[arity - 1] == last)) {
                if (found != null) {
                    throw new IllegalStateException(cls + "." + name + "/" + arity + " is ambiguous");
                }
                found = m;
            }
        }
        if (found == null) {
            throw new IllegalStateException(cls + "." + name + "/" + arity + " not found");
        }
        found.setAccessible(true);
        return found;
    }

    private static String chain(Class<?> c, Class<?> stop) {
        StringBuilder s = new StringBuilder();
        for (Class<?> k = c; k != null && k != stop; k = k.getSuperclass()) {
            if (s.length() > 0) {
                s.append(',');
            }
            s.append(k.getName());
        }
        return s.toString();
    }

    private static void assignable(String[] a) throws Exception {
        ClassLoader loader = RedstoneDump.class.getClassLoader();
        Class<?> target = Class.forName(a[1], false, loader);
        int yes = 0;
        int all = 0;
        PrintWriter out = new PrintWriter(new BufferedWriter(new FileWriter(a[3])));
        for (String name : Files.readAllLines(Paths.get(a[2]), StandardCharsets.UTF_8)) {
            if (name.isEmpty()) {
                continue;
            }
            boolean is = target.isAssignableFrom(Class.forName(name, false, loader));
            out.println(name + "\t" + is);
            all++;
            if (is) {
                yes++;
            }
        }
        out.close();
        System.out.println("ASSIGNABLE names=" + all + " assignable=" + yes);
    }

    public static void main(String[] a) throws Exception {
        if (a[0].equals("assignable")) {
            assignable(a);
            return;
        }
        if (!a[0].equals("states")) {
            throw new IllegalArgumentException("unknown mode " + a[0]);
        }
        staticNoArg(a[3], a[4]).invoke(null); // SharedConstants.tryDetectVersion()
        staticNoArg(a[5], a[6]).invoke(null); // Bootstrap.bootStrap()

        Class<?> direction = Class.forName(a[20]);
        Class<?> blockPos = Class.forName(a[18]);
        Object stateRegistry = staticField(a[7], a[8]); // Block.BLOCK_STATE_REGISTRY
        Method faceFull = byArity(a[7], a[9], 2, direction); // Block.isFaceFull(VoxelShape, Direction)
        Method collision = byArity(a[10], a[11], 2, null); // getCollisionShape(BlockGetter, BlockPos)
        Method conductor = byArity(a[10], a[12], 2, blockPos); // isRedstoneConductor(BlockGetter, BlockPos)
        Method shapeFull = byArity(a[10], a[13], 2, blockPos); // isCollisionShapeFullBlock(BlockGetter, BlockPos)
        Method source = byArity(a[10], a[14], 0, null); // isSignalSource()
        Method getBlock = byArity(a[10], a[15], 0, null); // getBlock()
        Object level = staticField(a[16], a[17]); // EmptyBlockGetter.INSTANCE
        Object zero = staticField(a[18], a[19]); // BlockPos.ZERO
        Object[] dirs = new Object[6];
        for (int i = 0; i < 6; i++) {
            dirs[i] = staticField(a[20], a[21 + i]);
        }
        Class<?> behaviour = Class.forName(a[27]);
        Class<?> entityBlock = Class.forName(a[28]);
        Method newBlockEntity = null;
        for (Method m : entityBlock.getDeclaredMethods()) {
            if (m.getName().equals(a[29]) && m.getParameterCount() == 2 && m.getParameterTypes()[0] == blockPos) {
                if (newBlockEntity != null) {
                    throw new IllegalStateException("EntityBlock.newBlockEntity is ambiguous");
                }
                newBlockEntity = m;
            }
        }
        if (newBlockEntity == null) {
            throw new IllegalStateException("EntityBlock.newBlockEntity not found");
        }
        Class<?> blockEntity = Class.forName(a[30]);

        int states = 0;
        int conductors = 0;
        int sources = 0;
        int entityBlocks = 0;
        int madeNone = 0;
        Map<Object, String> blocks = new LinkedHashMap<>();
        PrintWriter out = new PrintWriter(new BufferedWriter(new FileWriter(a[1])));
        for (Object st : (Iterable<?>) stateRegistry) {
            Object shape = collision.invoke(st, level, zero);
            boolean faces = true;
            for (int i = 0; i < 6; i++) {
                if (!(Boolean) faceFull.invoke(null, shape, dirs[i])) {
                    faces = false;
                }
            }
            boolean c = (Boolean) conductor.invoke(st, level, zero);
            boolean f = (Boolean) shapeFull.invoke(st, level, zero);
            boolean s = (Boolean) source.invoke(st);
            out.println(st + "\t" + c + "\t" + f + "\t" + faces + "\t" + s);
            states++;
            if (c) {
                conductors++;
            }
            if (s) {
                sources++;
            }
            Object block = getBlock.invoke(st);
            if (!blocks.containsKey(block)) {
                String be = "-";
                if (entityBlock.isInstance(block)) {
                    Object made = newBlockEntity.invoke(block, zero, st);
                    if (made == null) {
                        // `moving_piston`: its block entity is made by the piston, not by
                        // `newBlockEntity`; named so the driver can state the count.
                        be = "!";
                        madeNone++;
                    } else {
                        be = chain(made.getClass(), blockEntity);
                        entityBlocks++;
                    }
                }
                blocks.put(block, st + "\t" + chain(block.getClass(), behaviour) + "\t" + be);
            }
        }
        out.close();
        PrintWriter bout = new PrintWriter(new BufferedWriter(new FileWriter(a[2])));
        for (String line : blocks.values()) {
            bout.println(line);
        }
        bout.close();

        // The dumper's own binding count, so a truncated run cannot read as a clean
        // one. The driver asserts every number is non-zero.
        System.out.println("DUMPED states=" + states + " blocks=" + blocks.size() + " conductors=" + conductors
                + " sources=" + sources + " entity-blocks=" + entityBlocks + " entity-blocks-made-none=" + madeNone);
    }
}
