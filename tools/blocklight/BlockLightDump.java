// Dumps every blockstate's block-light emission out of a pinned Minecraft
// server jar, by booting the vanilla registries and calling the game's own
// `BlockState.getLightEmission()`.
//
// This file hardcodes NO obfuscated name. Every class/member it touches is
// passed in on the command line, resolved from the official Mojang mappings for
// the same pin by `tools/dump-block-light.py` — so a version bump changes the
// pin and nothing here.
//
// argv:
//   0  out: one line per blockstate, "<BlockState.toString()>\t<light>"
//   1  out: one line per block, "<id>\t<defaultState.toString()>\t<light>"
//   2  SharedConstants class            3  .tryDetectVersion()
//   4  Bootstrap class                  5  .bootStrap()
//   6  Block class                      7  .BLOCK_STATE_REGISTRY
//   8  Block.defaultBlockState()
//   9  BlockBehaviour$BlockStateBase   10  .getLightEmission()
//  11  BuiltInRegistries class         12  .BLOCK
//  13  Registry class                  14  .getKey(Object)
package dw;

import java.io.BufferedWriter;
import java.io.FileWriter;
import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;

public final class BlockLightDump {
    private BlockLightDump() {}

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

    public static void main(String[] a) throws Exception {
        staticNoArg(a[2], a[3]).invoke(null); // SharedConstants.tryDetectVersion()
        staticNoArg(a[4], a[5]).invoke(null); // Bootstrap.bootStrap()

        Object stateRegistry = staticField(a[6], a[7]); // Block.BLOCK_STATE_REGISTRY
        Method defaultState = Class.forName(a[6]).getDeclaredMethod(a[8]);
        defaultState.setAccessible(true);

        Method light = Class.forName(a[9]).getDeclaredMethod(a[10]); // getLightEmission()
        light.setAccessible(true);

        Object blockRegistry = staticField(a[11], a[12]); // BuiltInRegistries.BLOCK
        Method getKey = Class.forName(a[13]).getDeclaredMethod(a[14], Object.class);
        getKey.setAccessible(true);

        int states = 0;
        PrintWriter out = new PrintWriter(new BufferedWriter(new FileWriter(a[0])));
        for (Object st : (Iterable<?>) stateRegistry) {
            out.println(st + "\t" + light.invoke(st));
            states++;
        }
        out.close();

        int blocks = 0;
        PrintWriter out2 = new PrintWriter(new BufferedWriter(new FileWriter(a[1])));
        for (Object b : (Iterable<?>) blockRegistry) {
            Object ds = defaultState.invoke(b);
            out2.println(getKey.invoke(blockRegistry, b) + "\t" + ds + "\t" + light.invoke(ds));
            blocks++;
        }
        out2.close();

        // The dumper's own binding count, so a truncated run cannot read as a
        // clean one. The driver asserts both numbers are non-zero.
        System.out.println("DUMPED states=" + states + " blocks=" + blocks);
    }
}
