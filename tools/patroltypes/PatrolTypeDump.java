package dw;

import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;

/**
 * Ask the pinned Minecraft server jar which entity types honour patrol NBT.
 *
 * <p>`Patrolling` / `PatrolLeader` / `patrol_target` are read and written by
 * PatrollingMonster and — measured, not assumed — by nothing else in the jar, so
 * an entity type honours them exactly when the class it constructs is a
 * PatrollingMonster. The class each type constructs is read off the generic
 * signature of its static field on EntityType (`EntityType&lt;Pillager&gt;
 * PILLAGER`), which the shipped jar retains.
 *
 * <p>Every obfuscated name is passed in, resolved from the official mappings for
 * the same pin — none is written down here, so a version bump is a pin edit and
 * nothing else.
 *
 * <p>Output is one TSV row per entity type:
 * {@code <id>\t<constructed class>\t<patrolling|->\t<raider|->\t<super chain>},
 * where the chain is every class from the constructed one up to Object,
 * space-separated. The chain is what lets the caller ask a structural question —
 * is this carrier of the patrol NBT keys a class some entity is actually built
 * from — instead of exempting a class by its name.
 */
public final class PatrolTypeDump {
    public static void main(String[] args) throws Exception {
        int i = 0;
        String out = args[i++];
        String cShared = args[i++], mDetect = args[i++];
        String cBootstrap = args[i++], mBoot = args[i++];
        String cBuiltIn = args[i++], fEntityType = args[i++];
        String cRegistry = args[i++], mGetKey = args[i++];
        String cEntityType = args[i++];
        String cPatrolling = args[i++];
        String cRaider = args[i++];

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
        Class<?> patrolling = Class.forName(cPatrolling);
        Class<?> raider = Class.forName(cRaider);

        Map<Object, Class<?>> constructed = new IdentityHashMap<>();
        int erased = 0;
        for (Field f : entityType.getDeclaredFields()) {
            if (!entityType.isAssignableFrom(f.getType())) continue;
            f.setAccessible(true);
            Object value = f.get(null);
            if (value == null) continue;
            Type t = f.getGenericType();
            if (t instanceof ParameterizedType pt
                    && pt.getActualTypeArguments().length == 1
                    && pt.getActualTypeArguments()[0] instanceof Class<?> arg) {
                constructed.put(value, arg);
            } else {
                erased++;
            }
        }
        // A stripped Signature attribute would leave types unmapped and quietly
        // shrink the answer, which is the direction that reads as a pass.
        if (erased != 0) {
            throw new IllegalStateException(
                    erased + " EntityType field(s) carry no generic signature — "
                            + "this jar cannot be read this way");
        }

        List<String> rows = new ArrayList<>();
        int total = 0;
        for (Object type : (Iterable<?>) registry) {
            total++;
            Class<?> cls = constructed.get(type);
            if (cls == null) {
                throw new IllegalStateException(
                        "no EntityType field holds the registry entry " + getKey.invoke(registry, type));
            }
            StringBuilder chain = new StringBuilder();
            for (Class<?> c = cls; c != null; c = c.getSuperclass()) {
                if (chain.length() > 0) chain.append(' ');
                chain.append(c.getName());
            }
            rows.add(getKey.invoke(registry, type)
                    + "\t" + cls.getName()
                    + "\t" + (patrolling.isAssignableFrom(cls) ? "patrolling" : "-")
                    + "\t" + (raider.isAssignableFrom(cls) ? "raider" : "-")
                    + "\t" + chain);
        }
        rows.sort(String::compareTo);
        try (PrintWriter w = new PrintWriter(out, StandardCharsets.UTF_8)) {
            for (String r : rows) w.println(r);
        }
        System.out.println("DUMPED types=" + total);
    }
}
