// Dumps, for every blockstate of a pinned Minecraft server jar, which of its six
// faces are FULL — the question every block that hangs on another asks of the
// block it hangs on. Two answers per face, because vanilla asks two:
//
//   sturdy  `BlockState.isFaceSturdy(BlockGetter, BlockPos, Direction)` — the
//           three-argument form, `SupportType.FULL`, i.e. the face of the block's
//           SUPPORT shape is the whole square. A ladder asks this of the block
//           behind it; a weeping, twisting or cave vine of the block it grows from.
//   full    `Block.isFaceFull(getCollisionShape(BlockGetter, BlockPos), Direction)`
//           — the face of the COLLISION shape is the whole square. A vine accepts
//           either (`MultifaceBlock.canAttachTo`), which is why leaves, whose
//           support shape is empty, hold a vine and not a ladder.
//
// This file hardcodes NO obfuscated name. Every class/member it touches is passed
// in on the command line, resolved from the official Mojang mappings for the same
// pin by `tools/maintenance/dump-faces.py`.
//
// The faces are asked at `BlockPos.ZERO` in `EmptyBlockGetter.INSTANCE`, as the
// collision dumper asks its shapes.
//
// argv:
//   0  out: one line per blockstate, "<BlockState.toString()>\t<sturdy>\t<full>",
//      each a string over "dunswe" (down, up, north, south, west, east) naming the
//      faces that answer yes, "-" when none does
//   1  SharedConstants class            2  .tryDetectVersion()
//   3  Bootstrap class                  4  .bootStrap()
//   5  Block class                      6  .BLOCK_STATE_REGISTRY   7  .isFaceFull(VoxelShape, Direction)
//   8  BlockBehaviour$BlockStateBase    9  .getCollisionShape(BlockGetter, BlockPos)
//  10  .isFaceSturdy(BlockGetter, BlockPos, Direction)
//  11  EmptyBlockGetter class          12  .INSTANCE
//  13  BlockPos class                  14  .ZERO
//  15  Direction class                 16..21  .DOWN .UP .NORTH .SOUTH .WEST .EAST
package dw;

import java.io.BufferedWriter;
import java.io.FileWriter;
import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;

public final class FaceDump {
    private FaceDump() {}

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
                    && (last == null || m.getParameterTypes()[arity - 1] == last)) {
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

        Class<?> direction = Class.forName(a[15]);
        Object stateRegistry = staticField(a[5], a[6]); // Block.BLOCK_STATE_REGISTRY
        Method faceFull = byArity(a[5], a[7], 2, direction); // Block.isFaceFull(VoxelShape, Direction)
        Method collision = byArity(a[8], a[9], 2, null); // getCollisionShape(BlockGetter, BlockPos)
        Method sturdy = byArity(a[8], a[10], 3, direction); // isFaceSturdy(BlockGetter, BlockPos, Direction)
        Object level = staticField(a[11], a[12]); // EmptyBlockGetter.INSTANCE
        Object zero = staticField(a[13], a[14]); // BlockPos.ZERO
        String letters = "dunswe";
        Object[] dirs = new Object[6];
        for (int i = 0; i < 6; i++) {
            dirs[i] = staticField(a[15], a[16 + i]);
        }

        int states = 0;
        int anySturdy = 0;
        int anyFull = 0;
        PrintWriter out = new PrintWriter(new BufferedWriter(new FileWriter(a[0])));
        for (Object st : (Iterable<?>) stateRegistry) {
            Object shape = collision.invoke(st, level, zero);
            StringBuilder s = new StringBuilder();
            StringBuilder f = new StringBuilder();
            for (int i = 0; i < 6; i++) {
                if ((Boolean) sturdy.invoke(st, level, zero, dirs[i])) {
                    s.append(letters.charAt(i));
                }
                if ((Boolean) faceFull.invoke(null, shape, dirs[i])) {
                    f.append(letters.charAt(i));
                }
            }
            if (s.length() > 0) {
                anySturdy++;
            }
            if (f.length() > 0) {
                anyFull++;
            }
            out.println(st + "\t" + (s.length() == 0 ? "-" : s) + "\t" + (f.length() == 0 ? "-" : f));
            states++;
        }
        out.close();

        // The dumper's own binding count, so a truncated run cannot read as a clean
        // one. The driver asserts every number is non-zero.
        System.out.println("DUMPED states=" + states + " any-sturdy=" + anySturdy + " any-full=" + anyFull);
    }
}
