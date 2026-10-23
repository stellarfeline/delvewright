// Dumps, for every blockstate of a pinned Minecraft server jar, whether the
// block hides what stands behind it from the eye: the game's own
// `BlockState.canOcclude()`, whether its occlusion shape fills the cell
// (`Block.isShapeFullBlock(BlockState.getOcclusionShape())`), and the cached
// `BlockState.isSolidRender()` the renderer culls a neighbour's face by.
//
// This file hardcodes NO obfuscated name. Every class/member it touches is
// passed in on the command line, resolved from the official Mojang mappings for
// the same pin by `tools/maintenance/dump-block-light.py`.
//
// argv:
//   0  out: one line per blockstate,
//      "<BlockState.toString()>\t<canOcclude>\t<fullOcclusionShape>\t<solidRender>"
//   1  SharedConstants class            2  .tryDetectVersion()
//   3  Bootstrap class                  4  .bootStrap()
//   5  Block class                      6  .BLOCK_STATE_REGISTRY
//   7  Block.isShapeFullBlock(VoxelShape)
//   8  BlockBehaviour$BlockStateBase    9  .canOcclude()
//  10  .getOcclusionShape()            11  .isSolidRender()
//  12  VoxelShape class
package dw;

import java.io.BufferedWriter;
import java.io.FileWriter;
import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;

public final class OcclusionDump {
    private OcclusionDump() {}

    private static Method noArg(String cls, String name) throws Exception {
        Method m = Class.forName(cls).getDeclaredMethod(name);
        m.setAccessible(true);
        return m;
    }

    public static void main(String[] a) throws Exception {
        noArg(a[1], a[2]).invoke(null); // SharedConstants.tryDetectVersion()
        noArg(a[3], a[4]).invoke(null); // Bootstrap.bootStrap()

        Field reg = Class.forName(a[5]).getDeclaredField(a[6]);
        reg.setAccessible(true);
        Object stateRegistry = reg.get(null); // Block.BLOCK_STATE_REGISTRY

        Method fullBlock = Class.forName(a[5]).getDeclaredMethod(a[7], Class.forName(a[12]));
        fullBlock.setAccessible(true);
        Method canOcclude = noArg(a[8], a[9]);
        Method occlusionShape = noArg(a[8], a[10]);
        Method solidRender = noArg(a[8], a[11]);

        int states = 0;
        int hiding = 0;
        PrintWriter out = new PrintWriter(new BufferedWriter(new FileWriter(a[0])));
        for (Object st : (Iterable<?>) stateRegistry) {
            boolean occ = (Boolean) canOcclude.invoke(st);
            boolean full = (Boolean) fullBlock.invoke(null, occlusionShape.invoke(st));
            boolean solid = (Boolean) solidRender.invoke(st);
            out.println(st + "\t" + occ + "\t" + full + "\t" + solid);
            states++;
            if (occ && full) {
                hiding++;
            }
        }
        out.close();

        // The dumper's own binding count, so a truncated run cannot read as a
        // clean one. The driver asserts both numbers are non-zero.
        System.out.println("DUMPED states=" + states + " hiding=" + hiding);
    }
}
