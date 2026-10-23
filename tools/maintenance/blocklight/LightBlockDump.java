// Dumps, for every blockstate of a pinned Minecraft server jar, how much light
// the block takes from light passing through it: the game's own
// `BlockState.getLightBlock()` (0 to 15; 15 stops light) and
// `BlockState.useShapeForLightOcclusion()` (whether the light engine also
// stops light at the faces its occlusion shape covers, as a slab's or a
// stair's does).
//
// This file hardcodes NO obfuscated name. Every class/member it touches is
// passed in on the command line, resolved from the official Mojang mappings for
// the same pin by `tools/maintenance/dump-block-light.py`.
//
// argv:
//   0  out: one line per blockstate,
//      "<BlockState.toString()>\t<lightBlock>\t<useShapeForLightOcclusion>"
//   1  SharedConstants class            2  .tryDetectVersion()
//   3  Bootstrap class                  4  .bootStrap()
//   5  Block class                      6  .BLOCK_STATE_REGISTRY
//   7  BlockBehaviour$BlockStateBase    8  .getLightBlock()
//   9  .useShapeForLightOcclusion()
package dw;

import java.io.BufferedWriter;
import java.io.FileWriter;
import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;

public final class LightBlockDump {
    private LightBlockDump() {}

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

        Method lightBlock = noArg(a[7], a[8]);
        Method useShape = noArg(a[7], a[9]);

        int states = 0;
        int stopping = 0;
        PrintWriter out = new PrintWriter(new BufferedWriter(new FileWriter(a[0])));
        for (Object st : (Iterable<?>) stateRegistry) {
            int lb = (Integer) lightBlock.invoke(st);
            boolean shape = (Boolean) useShape.invoke(st);
            out.println(st + "\t" + lb + "\t" + shape);
            states++;
            if (lb >= 15) {
                stopping++;
            }
        }
        out.close();

        // The dumper's own binding count, so a truncated run cannot read as a
        // clean one. The driver asserts both numbers are non-zero.
        System.out.println("DUMPED states=" + states + " stopping=" + stopping);
    }
}
