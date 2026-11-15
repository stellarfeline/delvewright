package dw;

import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

/**
 * Ask the pinned Minecraft server jar, per entity type, which mob category it
 * belongs to and the base value of its {@code minecraft:follow_range}.
 *
 * <p>For every registered entity type it reports {@code EntityType#getCategory()}
 * by its serialised name, and — where {@code DefaultAttributes#hasSupplier} holds
 * and the supplier carries the attribute — {@code AttributeSupplier#getBaseValue}
 * of {@code Attributes.FOLLOW_RANGE}: the value {@code attribute ... base reset}
 * restores. The caller writes the table; this dumper decides nothing.
 *
 * <p>Every obfuscated name is passed in, resolved from the official mappings for
 * the same pin — none is written down here.
 *
 * <p>Output: one TSV row per entity type, {@code <id>\t<category>\t<follow_range|->}.
 */
public final class EngagementDump {
    /** The method {@code name} of {@code c} taking no argument. */
    private static Method named(Class<?> c, String name) throws NoSuchMethodException {
        Method m = c.getDeclaredMethod(name);
        m.setAccessible(true);
        return m;
    }

    /** The one method {@code name} of {@code c} whose single parameter takes {@code arg}. */
    private static Method taking(Class<?> c, String name, Object arg) throws NoSuchMethodException {
        Method found = null;
        for (Method m : c.getDeclaredMethods()) {
            if (m.getName().equals(name) && m.getParameterCount() == 1
                    && m.getParameterTypes()[0].isInstance(arg)) {
                if (found != null) {
                    throw new NoSuchMethodException(c.getName() + "." + name + " is ambiguous");
                }
                found = m;
            }
        }
        if (found == null) throw new NoSuchMethodException(c.getName() + "." + name + "(1)");
        found.setAccessible(true);
        return found;
    }

    public static void main(String[] args) throws Exception {
        int i = 0;
        String out = args[i++];
        String cShared = args[i++], mDetect = args[i++];
        String cBootstrap = args[i++], mBoot = args[i++];
        String cBuiltIn = args[i++], fEntityType = args[i++];
        String cRegistry = args[i++], mGetKey = args[i++];
        String cEntityType = args[i++], mCategory = args[i++];
        String cCategory = args[i++], mCategoryName = args[i++];
        String cDefaults = args[i++], mHasSupplier = args[i++], mSupplier = args[i++];
        String cSupplier = args[i++], mHas = args[i++], mBase = args[i++];
        String cAttributes = args[i++], fFollow = args[i++];

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

        Method category = named(Class.forName(cEntityType), mCategory);
        Method categoryName = named(Class.forName(cCategory), mCategoryName);
        Class<?> defaults = Class.forName(cDefaults);
        Class<?> sup = Class.forName(cSupplier);
        Field follow = Class.forName(cAttributes).getDeclaredField(fFollow);
        follow.setAccessible(true);
        Object followRange = follow.get(null);
        Object anyType = ((Iterable<?>) registry).iterator().next();
        Method hasSupplier = taking(defaults, mHasSupplier, anyType);
        Method supplier = taking(defaults, mSupplier, anyType);
        Method has = taking(sup, mHas, followRange);
        Method base = taking(sup, mBase, followRange);

        List<String> rows = new ArrayList<>();
        int total = 0;
        int ranged = 0;
        for (Object type : (Iterable<?>) registry) {
            total++;
            String range = "-";
            if ((Boolean) hasSupplier.invoke(null, type)) {
                Object s = supplier.invoke(null, type);
                if ((Boolean) has.invoke(s, followRange)) {
                    range = String.valueOf(base.invoke(s, followRange));
                    ranged++;
                }
            }
            rows.add(getKey.invoke(registry, type)
                    + "\t" + categoryName.invoke(category.invoke(type))
                    + "\t" + range);
        }
        rows.sort(String::compareTo);
        try (PrintWriter w = new PrintWriter(out, StandardCharsets.UTF_8)) {
            for (String r : rows) w.println(r);
        }
        System.out.println("DUMPED types=" + total + " ranged=" + ranged);
    }
}
