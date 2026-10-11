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
 * Ask the pinned Minecraft server jar what peaceful does to each entity type.
 *
 * <p>For every registered entity type it reports the type's own
 * {@code EntityType#isAllowedInPeaceful()} flag, the class the type constructs
 * (read off the generic signature of its static field on EntityType, as
 * {@code PatrolTypeDump} does), and the class that declares the
 * {@code Entity#checkDespawn()} implementation that body runs. The caller turns
 * those three facts into a verdict; this dumper decides nothing.
 *
 * <p>Every obfuscated name is passed in, resolved from the official mappings for
 * the same pin — none is written down here.
 *
 * <p>Output: one TSV row per entity type,
 * {@code <id>\t<allowed|not_allowed>\t<constructed class>\t<checkDespawn declarer>}.
 */
public final class PeacefulDump {
    public static void main(String[] args) throws Exception {
        int i = 0;
        String out = args[i++];
        String cShared = args[i++], mDetect = args[i++];
        String cBootstrap = args[i++], mBoot = args[i++];
        String cBuiltIn = args[i++], fEntityType = args[i++];
        String cRegistry = args[i++], mGetKey = args[i++];
        String cEntityType = args[i++], mAllowed = args[i++];
        String mCheckDespawn = args[i++];

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
        Method allowed = entityType.getDeclaredMethod(mAllowed);
        allowed.setAccessible(true);

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
            Class<?> declarer = null;
            for (Class<?> c = cls; c != null && declarer == null; c = c.getSuperclass()) {
                for (Method m : c.getDeclaredMethods()) {
                    if (m.getName().equals(mCheckDespawn) && m.getParameterCount() == 0
                            && m.getReturnType() == void.class) {
                        declarer = c;
                        break;
                    }
                }
            }
            if (declarer == null) {
                throw new IllegalStateException(
                        "no checkDespawn() on the chain of " + cls.getName());
            }
            boolean ok = (Boolean) allowed.invoke(type);
            rows.add(getKey.invoke(registry, type)
                    + "\t" + (ok ? "allowed" : "not_allowed")
                    + "\t" + cls.getName()
                    + "\t" + declarer.getName());
        }
        rows.sort(String::compareTo);
        try (PrintWriter w = new PrintWriter(out, StandardCharsets.UTF_8)) {
            for (String r : rows) w.println(r);
        }
        System.out.println("DUMPED types=" + total);
    }
}
