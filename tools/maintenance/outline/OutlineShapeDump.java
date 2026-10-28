// Dumps every blockstate's OUTLINE shape out of a pinned Minecraft server jar:
// the boxes of the shape the game's own `BlockState.getShape(BlockGetter,
// BlockPos)` returns — the shape the crosshair's pick hits and the client draws
// the selection box around — read box by box (`VoxelShape.toAabbs()`).
//
// An outline is not a collision shape: a lever and every button have no
// collision and a real outline, and a door's outline is its leaf whether or not
// the leaf is open. `tools/maintenance/collision/` dumps the collision shape;
// this dumps the other method.
//
// This file hardcodes NO obfuscated name; every class/member is passed in on the
// command line, resolved from the official Mojang mappings for the same pin by
// `tools/maintenance/dump-outline-shapes.py`.
//
// The shape is asked at `BlockPos.ZERO` in `EmptyBlockGetter.INSTANCE`, with the
// empty collision context the two-argument `getShape` supplies: no neighbour
// exists to change it, and a block with a random offset is offset by the same
// fixed position every run.
//
// argv:
//   0  out: one line per blockstate, "<BlockState.toString()>\t<boxes>"
//      boxes = "-" (empty), else ";"-joined "x0,y0,z0,x1,y1,z1", each bound the
//      exact double the box holds, sorted, so the line is deterministic.
//   1  SharedConstants class            2  .tryDetectVersion()
//   3  Bootstrap class                  4  .bootStrap()
//   5  Block class                      6  .BLOCK_STATE_REGISTRY
//   7  BlockBehaviour$BlockStateBase    8  .getShape(BlockGetter, BlockPos)
//   9  EmptyBlockGetter class          10  .INSTANCE
//  11  BlockPos class                  12  .ZERO
//  13  VoxelShape class                14  .toAabbs()   15  .isEmpty()
//  16  AABB class                      17  .minX  18  .minY  19  .minZ
//                                      20  .maxX  21  .maxY  22  .maxZ
package dw;

import java.io.BufferedWriter;
import java.io.FileWriter;
import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

public final class OutlineShapeDump {
    private OutlineShapeDump() {}

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

    /**
     * The one declared method of `cls` called `name` taking `arity` arguments; an
     * ambiguity is refused rather than resolved by declaration order.
     */
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
        staticNoArg(a[1], a[2]).invoke(null); // SharedConstants.tryDetectVersion()
        staticNoArg(a[3], a[4]).invoke(null); // Bootstrap.bootStrap()

        Object stateRegistry = staticField(a[5], a[6]); // Block.BLOCK_STATE_REGISTRY
        Method shapeOf = byArity(a[7], a[8], 2); // getShape(BlockGetter, BlockPos)
        Object level = staticField(a[9], a[10]); // EmptyBlockGetter.INSTANCE
        Object zero = staticField(a[11], a[12]); // BlockPos.ZERO
        Method toAabbs = byArity(a[13], a[14], 0);
        Method empty = byArity(a[13], a[15], 0);
        Field[] bound = {
            field(a[16], a[17]), field(a[16], a[18]), field(a[16], a[19]),
            field(a[16], a[20]), field(a[16], a[21]), field(a[16], a[22]),
        };

        int states = 0;
        int outlined = 0;
        PrintWriter out = new PrintWriter(new BufferedWriter(new FileWriter(a[0])));
        for (Object st : (Iterable<?>) stateRegistry) {
            states++;
            Object shape = shapeOf.invoke(st, level, zero);
            if ((Boolean) empty.invoke(shape)) {
                out.println(st + "\t-");
                continue;
            }
            List<String> boxes = new ArrayList<>();
            for (Object box : (List<?>) toAabbs.invoke(shape)) {
                StringBuilder sb = new StringBuilder();
                for (int i = 0; i < 6; i++) {
                    if (i > 0) {
                        sb.append(',');
                    }
                    sb.append(Double.toString(bound[i].getDouble(box)));
                }
                boxes.add(sb.toString());
            }
            Collections.sort(boxes);
            out.println(st + "\t" + String.join(";", boxes));
            outlined++;
        }
        out.close();

        // The dumper's own binding count, so a truncated run cannot read as a
        // clean one. The driver asserts both numbers are non-zero.
        System.out.println("DUMPED states=" + states + " with-outline=" + outlined);
    }
}
