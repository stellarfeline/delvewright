package dw;

import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

/**
 * Ask the pinned Minecraft server jar which particle types exist and which of
 * them take options.
 *
 * <p>A particle type that takes no options is a {@code SimpleParticleType}: the
 * type object is itself the {@code ParticleOptions} the {@code particle} command
 * sends, so {@code particle <id>} needs nothing after the id. Every other type
 * carries a codec for options ({@code dust}'s colour, {@code block}'s state, …)
 * and is not itself a {@code ParticleOptions}. Both readings are taken — the
 * concrete class and the interface — and the caller requires them to agree, so a
 * future type that is one and not the other is a disagreement, never a guess.
 *
 * <p>Every obfuscated name is passed in, resolved from the official mappings for
 * the same pin — none is written down here, so a version bump is a pin edit and
 * nothing else.
 *
 * <p>Output is one TSV row per particle type, sorted:
 * {@code <id>\t<simple|options>\t<is-options|not-options>}.
 */
public final class ParticleTypeDump {
    public static void main(String[] args) throws Exception {
        int i = 0;
        String out = args[i++];
        String cShared = args[i++], mDetect = args[i++];
        String cBootstrap = args[i++], mBoot = args[i++];
        String cBuiltIn = args[i++], fParticleType = args[i++];
        String cRegistry = args[i++], mGetKey = args[i++];
        String cSimple = args[i++];
        String cOptions = args[i++];

        Method detect = Class.forName(cShared).getDeclaredMethod(mDetect);
        detect.setAccessible(true);
        detect.invoke(null);
        Method boot = Class.forName(cBootstrap).getDeclaredMethod(mBoot);
        boot.setAccessible(true);
        boot.invoke(null);

        Field reg = Class.forName(cBuiltIn).getDeclaredField(fParticleType);
        reg.setAccessible(true);
        Object registry = reg.get(null);
        Method getKey = Class.forName(cRegistry).getMethod(mGetKey, Object.class);
        getKey.setAccessible(true);

        Class<?> simple = Class.forName(cSimple);
        Class<?> options = Class.forName(cOptions);

        List<String> rows = new ArrayList<>();
        int total = 0;
        for (Object type : (Iterable<?>) registry) {
            total++;
            Object key = getKey.invoke(registry, type);
            if (key == null) {
                throw new IllegalStateException("a particle type has no registry key: " + type);
            }
            rows.add(key
                    + "\t" + (simple.isInstance(type) ? "simple" : "options")
                    + "\t" + (options.isInstance(type) ? "is-options" : "not-options"));
        }
        rows.sort(String::compareTo);
        try (PrintWriter w = new PrintWriter(out, StandardCharsets.UTF_8)) {
            for (String r : rows) w.println(r);
        }
        System.out.println("DUMPED types=" + total);
    }
}
