// Dumps every blockstate's collision FOOTPRINT out of a pinned Minecraft server
// jar: the horizontal (x, z) projection of the shape the game's own
// `BlockState.getCollisionShape(BlockGetter, BlockPos)` returns, as a 16x16 grid
// of sixteenths. A sixteenth-square is covered when it lies wholly inside one of
// the shape's boxes (`VoxelShape.toAabbs()`), so a box bound that is not a whole
// sixteenth covers less, never more: the grid can only report a gap vanilla
// closes, never close one vanilla leaves open.
//
// This file hardcodes NO obfuscated name; every class/member is passed in on the
// command line, resolved from the official Mojang mappings for the same pin by
// `tools/maintenance/dump-collision-footprints.py`.
//
// argv:
//   0  out: one line per blockstate, "<BlockState.toString()>\t<footprint>"
//      footprint = "-" (empty), "full" (every sixteenth covered), or sixteen
//      4-hex-digit rows, z = 0..15, bit x set when (x, z) is covered.
//   1  SharedConstants class            2  .tryDetectVersion()
//   3  Bootstrap class                  4  .bootStrap()
//   5  Block class                      6  .BLOCK_STATE_REGISTRY
//   7  BlockBehaviour$BlockStateBase    8  .getCollisionShape(BlockGetter, BlockPos)
//   9  EmptyBlockGetter class          10  .INSTANCE
//  11  BlockPos class                  12  .ZERO
//  13  VoxelShape class                14  .toAabbs()   15  .isEmpty()
//  16  AABB class                      17  .minX  18  .minZ  19  .maxX  20  .maxZ
package dw;

import java.io.BufferedWriter;
import java.io.FileWriter;
import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.List;

public final class CollisionFootprintDump {
    private CollisionFootprintDump() {}

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

    private static Field field(String cls, String name) throws Exception {
        Field f = Class.forName(cls).getDeclaredField(name);
        f.setAccessible(true);
        return f;
    }

    private static Method byArity(String cls, String name, int arity) throws Exception {
        Method found = null;
        for (Method m : Class.forName(cls).getDeclaredMethods()) {
            if (m.getName().equals(name) && m.getParameterCount() == arity) {
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
        staticNoArg(a[1], a[2]).invoke(null);
        staticNoArg(a[3], a[4]).invoke(null);

        Object stateRegistry = staticField(a[5], a[6]);
        Method collision = byArity(a[7], a[8], 2);
        Object level = staticField(a[9], a[10]);
        Object zero = staticField(a[11], a[12]);
        Method toAabbs = byArity(a[13], a[14], 0);
        Method empty = byArity(a[13], a[15], 0);
        Field minX = field(a[16], a[17]);
        Field minZ = field(a[16], a[18]);
        Field maxX = field(a[16], a[19]);
        Field maxZ = field(a[16], a[20]);

        int states = 0;
        int partial = 0;
        PrintWriter out = new PrintWriter(new BufferedWriter(new FileWriter(a[0])));
        for (Object st : (Iterable<?>) stateRegistry) {
            states++;
            Object shape = collision.invoke(st, level, zero);
            if ((Boolean) empty.invoke(shape)) {
                out.println(st + "\t-");
                continue;
            }
            int[] rows = new int[16];
            for (Object box : (List<?>) toAabbs.invoke(shape)) {
                double x0 = minX.getDouble(box) * 16, x1 = maxX.getDouble(box) * 16;
                double z0 = minZ.getDouble(box) * 16, z1 = maxZ.getDouble(box) * 16;
                for (int z = 0; z < 16; z++) {
                    if (z < z0 - 1e-9 || z + 1 > z1 + 1e-9) {
                        continue;
                    }
                    for (int x = 0; x < 16; x++) {
                        if (x >= x0 - 1e-9 && x + 1 <= x1 + 1e-9) {
                            rows[z] |= 1 << x;
                        }
                    }
                }
            }
            boolean full = true;
            StringBuilder sb = new StringBuilder();
            for (int z = 0; z < 16; z++) {
                full &= rows[z] == 0xFFFF;
                sb.append(String.format("%04x", rows[z]));
            }
            if (full) {
                out.println(st + "\tfull");
            } else {
                out.println(st + "\t" + sb);
                partial++;
            }
        }
        out.close();
        System.out.println("DUMPED states=" + states + " partial-footprint=" + partial);
    }
}
