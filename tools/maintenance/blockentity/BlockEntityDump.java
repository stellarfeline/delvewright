// Dumps, for every blockstate of a pinned Minecraft server jar, the block entity
// type the game makes for it, for `tools/maintenance/dump-block-entities.py`.
//
// A block that is an `EntityBlock` gets a block entity whenever it is placed —
// by a structure template, a `setblock`, a `fill` — whether or not the placing
// write carries a compound for it, and the server saves that block entity with
// the chunk. Its type is `EntityBlock.newBlockEntity(BlockPos.ZERO, state).getType()`,
// named by `BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(type)`.
//
// This file hardcodes NO obfuscated name. Every class/member it touches is passed
// in on the command line, resolved from the official Mojang mappings for the same
// pin by `tools/maintenance/dump-block-entities.py`.
//
// argv:
//   0  out: one line per blockstate, "<BlockState.toString()>\t<type id>", the type
//      id "-" for a block that is no EntityBlock and "!" for one whose
//      newBlockEntity(BlockPos.ZERO, state) returns null
//   1  SharedConstants class            2  .tryDetectVersion()
//   3  Bootstrap class                  4  .bootStrap()
//   5  Block class                      6  .BLOCK_STATE_REGISTRY
//   7  BlockBehaviour$BlockStateBase    8  .getBlock()
//   9  BlockPos class                  10  .ZERO
//  11  EntityBlock interface           12  .newBlockEntity(BlockPos, BlockState)
//  13  BlockEntity class               14  .getType()
//  15  BuiltInRegistries class         16  .BLOCK_ENTITY_TYPE
//  17  Registry interface              18  .getKey(Object)
package dw;

import java.io.BufferedWriter;
import java.io.FileWriter;
import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;

public final class BlockEntityDump {
    private BlockEntityDump() {}

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
     * whose FIRST parameter is `first` (or any, when null). An ambiguity is
     * refused rather than resolved by declaration order.
     */
    private static Method byArity(String cls, String name, int arity, Class<?> first) throws Exception {
        Method found = null;
        for (Method m : Class.forName(cls).getDeclaredMethods()) {
            if (m.getName().equals(name) && m.getParameterCount() == arity && !m.isBridge()
                    && (arity == 0 || first == null || m.getParameterTypes()[0] == first)) {
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
        Method getBlock = byArity(a[7], a[8], 0, null); // getBlock()
        Class<?> blockPos = Class.forName(a[9]);
        Object zero = staticField(a[9], a[10]); // BlockPos.ZERO
        Class<?> entityBlock = Class.forName(a[11]);
        Method newBlockEntity = byArity(a[11], a[12], 2, blockPos); // newBlockEntity(BlockPos, BlockState)
        Method getType = byArity(a[13], a[14], 0, null); // BlockEntity.getType()
        Object types = staticField(a[15], a[16]); // BuiltInRegistries.BLOCK_ENTITY_TYPE
        Method getKey = byArity(a[17], a[18], 1, Object.class); // Registry.getKey(Object)

        int states = 0;
        int entityStates = 0;
        int madeNone = 0;
        PrintWriter out = new PrintWriter(new BufferedWriter(new FileWriter(a[0])));
        for (Object st : (Iterable<?>) stateRegistry) {
            Object block = getBlock.invoke(st);
            String type = "-";
            if (entityBlock.isInstance(block)) {
                Object made = newBlockEntity.invoke(block, zero, st);
                if (made == null) {
                    // `moving_piston`: its block entity is made by the piston, not by
                    // `newBlockEntity`; named so the driver can state the count.
                    type = "!";
                    madeNone++;
                } else {
                    Object key = getKey.invoke(types, getType.invoke(made));
                    if (key == null) {
                        throw new IllegalStateException(st + ": its block entity type has no registry key");
                    }
                    type = key.toString();
                    entityStates++;
                }
            }
            out.println(st + "\t" + type);
            states++;
        }
        out.close();

        // The dumper's own binding count, so a truncated run cannot read as a clean
        // one. The driver asserts the counts are non-zero.
        System.out.println("DUMPED states=" + states + " entity-states=" + entityStates
                + " entity-states-made-none=" + madeNone);
    }
}
