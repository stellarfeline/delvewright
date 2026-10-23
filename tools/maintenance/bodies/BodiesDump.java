package dw;

import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

/**
 * Ask the pinned Minecraft server jar how big each entity type's body is and
 * how far the server tracks it.
 *
 * <p>For every registered entity type it reports {@code EntityType#getWidth()},
 * {@code EntityType#getHeight()} and {@code EntityType#clientTrackingRange()}
 * (in chunks). The caller writes the table; this dumper decides nothing.
 *
 * <p>Every obfuscated name is passed in, resolved from the official mappings for
 * the same pin — none is written down here.
 *
 * <p>Output: one TSV row per entity type, {@code <id>\t<width>\t<height>\t<range>}.
 */
public final class BodiesDump {
    public static void main(String[] args) throws Exception {
        int i = 0;
        String out = args[i++];
        String cShared = args[i++], mDetect = args[i++];
        String cBootstrap = args[i++], mBoot = args[i++];
        String cBuiltIn = args[i++], fEntityType = args[i++];
        String cRegistry = args[i++], mGetKey = args[i++];
        String cEntityType = args[i++], mWidth = args[i++], mHeight = args[i++], mRange = args[i++];

        Method detect = Class.forName(cShared).getDeclaredMethod(mDetect);
        detect.setAccessible(true);
        detect.invoke(null);
        Method boot = Class.forName(cBootstrap).getDeclaredMethod(mBoot);
        boot.setAccessible(true);
        boot.invoke(null);

        Field reg = Class.forName(cBuiltIn).getDeclaredField(fEntityType);
        reg.setAccessible(true);
        Object registry = reg.get(null);
        Method getKey = Class.forName(cRegistry).getMethod(mGetKey, Object.class);
        getKey.setAccessible(true);

        Class<?> entityType = Class.forName(cEntityType);
        Method width = entityType.getDeclaredMethod(mWidth);
        Method height = entityType.getDeclaredMethod(mHeight);
        Method range = entityType.getDeclaredMethod(mRange);
        width.setAccessible(true);
        height.setAccessible(true);
        range.setAccessible(true);

        List<String> rows = new ArrayList<>();
        int total = 0;
        for (Object type : (Iterable<?>) registry) {
            total++;
            rows.add(getKey.invoke(registry, type)
                    + "\t" + width.invoke(type)
                    + "\t" + height.invoke(type)
                    + "\t" + range.invoke(type));
        }
        rows.sort(String::compareTo);
        try (PrintWriter w = new PrintWriter(out, StandardCharsets.UTF_8)) {
            for (String r : rows) w.println(r);
        }
        System.out.println("DUMPED types=" + total);
    }
}
