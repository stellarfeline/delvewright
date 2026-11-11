package dw;

import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

/**
 * Ask the pinned Minecraft server jar how long every item is held in use.
 *
 * <p>For every registered item it reports {@code Item#getUseDuration(ItemStack, LivingEntity)}
 * on the item's default stack ({@code Item#getDefaultInstance()}), with no entity. An item
 * whose answer is zero is never reported in use by the game. The caller writes the table;
 * this dumper decides nothing.
 *
 * <p>Every obfuscated name is passed in, resolved from the official mappings for the same
 * pin — none is written down here.
 *
 * <p>Output: one TSV row per item, {@code <id>\t<ticks>}, or {@code <id>\tholder:<exception>}
 * for an item whose answer needs the holding entity (it reads a registry through it).
 */
public final class UseDurationDump {
    public static void main(String[] args) throws Exception {
        int i = 0;
        String out = args[i++];
        String cShared = args[i++], mDetect = args[i++];
        String cBootstrap = args[i++], mBoot = args[i++];
        String cBuiltIn = args[i++], fItem = args[i++];
        String cRegistry = args[i++], mGetKey = args[i++];
        String cItem = args[i++], mDefault = args[i++], mUse = args[i++];
        String cStack = args[i++], cLiving = args[i++];

        Method detect = Class.forName(cShared).getDeclaredMethod(mDetect);
        detect.setAccessible(true);
        detect.invoke(null);
        Method boot = Class.forName(cBootstrap).getDeclaredMethod(mBoot);
        boot.setAccessible(true);
        boot.invoke(null);

        Field reg = Class.forName(cBuiltIn).getDeclaredField(fItem);
        reg.setAccessible(true);
        Object registry = reg.get(null);
        Method getKey = Class.forName(cRegistry).getMethod(mGetKey, Object.class);
        getKey.setAccessible(true);

        Class<?> item = Class.forName(cItem);
        Method defaultInstance = item.getDeclaredMethod(mDefault);
        defaultInstance.setAccessible(true);
        Method use = item.getMethod(mUse, Class.forName(cStack), Class.forName(cLiving));
        use.setAccessible(true);

        List<String> rows = new ArrayList<>();
        int total = 0;
        for (Object it : (Iterable<?>) registry) {
            total++;
            Object stack = defaultInstance.invoke(it);
            String answer;
            try {
                answer = String.valueOf(use.invoke(it, stack, null));
            } catch (InvocationTargetException e) {
                // The item reads its duration through the holder (a registry the
                // holder's level carries); the caller resolves it from the data.
                answer = "holder:" + e.getCause().getClass().getSimpleName();
            }
            rows.add(getKey.invoke(registry, it) + "\t" + answer);
        }
        rows.sort(String::compareTo);
        try (PrintWriter w = new PrintWriter(out, StandardCharsets.UTF_8)) {
            for (String r : rows) w.println(r);
        }
        System.out.println("DUMPED items=" + total);
    }
}
