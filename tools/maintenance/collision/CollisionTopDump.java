// Dumps every blockstate's collision box out of a pinned Minecraft server jar,
// by booting the vanilla registries and calling the game's own
// `BlockState.getCollisionShape(BlockGetter, BlockPos)` — the shape an entity's
// movement is resolved against, so its top face is where a standing body's feet
// are.
//
// This file hardcodes NO obfuscated name. Every class/member it touches is
// passed in on the command line, resolved from the official Mojang mappings for
// the same pin by `tools/maintenance/dump-collision-tops.py` — so a version bump
// changes the pin and nothing here.
//
// The shape is asked at `BlockPos.ZERO` in `EmptyBlockGetter.INSTANCE`: no
// neighbour exists to change it, and a block with a random horizontal offset
// (pointed dripstone, bamboo) is offset by the same fixed position every run.
// Only the vertical extent is written, and an offset is horizontal.
//
// argv:
//   0  out: one line per blockstate, "<BlockState.toString()>\t<ymin>\t<ymax>"
//      (blocks; "-" for both when the shape is empty)
//   1  SharedConstants class            2  .tryDetectVersion()
//   3  Bootstrap class                  4  .bootStrap()
//   5  Block class                      6  .BLOCK_STATE_REGISTRY
//   7  BlockBehaviour$BlockStateBase    8  .getCollisionShape(BlockGetter, BlockPos)
//   9  EmptyBlockGetter class          10  .INSTANCE
//  11  BlockPos class                  12  .ZERO
//  13  VoxelShape class                14  .min(Axis)   15  .max(Axis)   16  .isEmpty()
//  17  Direction$Axis class            18  .Y
package dw;

import java.io.BufferedWriter;
import java.io.FileWriter;
import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;

public final class CollisionTopDump {
    private CollisionTopDump() {}

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
     * The one declared method of `cls` called `name` taking `arity` arguments —
     * and, when `only` is given, whose single parameter is of that class. A
     * mapped name is shared by unrelated overloads, so arity alone is not always
     * an identity; an ambiguity is refused rather than resolved by order.
     */
    private static Method byArity(String cls, String name, int arity) throws Exception {
        return byArity(cls, name, arity, null);
    }

    private static Method byArity(String cls, String name, int arity, Class<?> only) throws Exception {
        Method found = null;
        for (Method m : Class.forName(cls).getDeclaredMethods()) {
            if (m.getName().equals(name) && m.getParameterCount() == arity
                    && (only == null || m.getParameterTypes()[0] == only)) {
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

    public static void main(String[] a) throws Exception {
        staticNoArg(a[1], a[2]).invoke(null); // SharedConstants.tryDetectVersion()
        staticNoArg(a[3], a[4]).invoke(null); // Bootstrap.bootStrap()

        Object stateRegistry = staticField(a[5], a[6]); // Block.BLOCK_STATE_REGISTRY
        Method collision = byArity(a[7], a[8], 2); // getCollisionShape(BlockGetter, BlockPos)
        Object level = staticField(a[9], a[10]); // EmptyBlockGetter.INSTANCE
        Object zero = staticField(a[11], a[12]); // BlockPos.ZERO
        Class<?> axis = Class.forName(a[17]);
        Method min = byArity(a[13], a[14], 1, axis);
        Method max = byArity(a[13], a[15], 1, axis);
        Method empty = byArity(a[13], a[16], 0);
        Object axisY = staticField(a[17], a[18]);

        int states = 0;
        int solid = 0;
        PrintWriter out = new PrintWriter(new BufferedWriter(new FileWriter(a[0])));
        for (Object st : (Iterable<?>) stateRegistry) {
            Object shape = collision.invoke(st, level, zero);
            if ((Boolean) empty.invoke(shape)) {
                out.println(st + "\t-\t-");
            } else {
                out.println(st + "\t" + min.invoke(shape, axisY) + "\t" + max.invoke(shape, axisY));
                solid++;
            }
            states++;
        }
        out.close();

        // The dumper's own binding count, so a truncated run cannot read as a
        // clean one. The driver asserts both numbers are non-zero.
        System.out.println("DUMPED states=" + states + " with-collision=" + solid);
    }
}
