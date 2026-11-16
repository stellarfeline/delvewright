package dw;

import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;

/**
 * Ask the pinned Minecraft server jar for the facts a body's variant is read from.
 *
 * <p>For every registered entity type it reports the class the type constructs and that
 * class's superclass chain (as {@code SteeringDump} does); for every static field of
 * {@code DataComponents} the component id it registers; for each enum the caller names, every
 * constant's {@code name()}, its {@code getSerializedName()} when it has one, and its numeric id
 * from the method the caller names; for each built-in registry the caller names, every key.
 * The caller turns those facts into the variant table; this dumper decides nothing.
 *
 * <p>Every obfuscated name is passed in, resolved from the official mappings for the same pin —
 * none is written down here.
 *
 * <p>Output rows (tab-separated): {@code TYPE <id> <class>,<super>,…}; {@code COMP <field> <id>};
 * {@code ENUM <label> <name()> <serialized or -> <id or ->}; {@code REG <label> <key>}.
 */
public final class VariantDump {
    public static void main(String[] args) throws Exception {
        int i = 0;
        String out = args[i++];
        String cShared = args[i++], mDetect = args[i++];
        String cBootstrap = args[i++], mBoot = args[i++];
        String cBuiltIn = args[i++], fEntityType = args[i++], fComponentType = args[i++];
        String cRegistry = args[i++], mGetKey = args[i++];
        String cEntityType = args[i++];
        String cComponents = args[i++];
        String mSerialized = args[i++];

        Method detect = Class.forName(cShared).getDeclaredMethod(mDetect);
        detect.setAccessible(true);
        detect.invoke(null);
        Method boot = Class.forName(cBootstrap).getDeclaredMethod(mBoot);
        boot.setAccessible(true);
        boot.invoke(null);

        Class<?> builtIn = Class.forName(cBuiltIn);
        Object entities = staticField(builtIn, fEntityType);
        Object components = staticField(builtIn, fComponentType);
        Method getKey = Class.forName(cRegistry).getMethod(mGetKey, Object.class);
        getKey.setAccessible(true);

        List<String> rows = new ArrayList<>();

        Class<?> entityType = Class.forName(cEntityType);
        Map<Object, Class<?>> constructed = new IdentityHashMap<>();
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
                throw new IllegalStateException("an EntityType field carries no generic signature");
            }
        }
        int types = 0;
        for (Object type : (Iterable<?>) entities) {
            types++;
            Class<?> cls = constructed.get(type);
            if (cls == null) {
                throw new IllegalStateException("no EntityType field holds " + getKey.invoke(entities, type));
            }
            StringBuilder chain = new StringBuilder();
            for (Class<?> c = cls; c != null; c = c.getSuperclass()) {
                if (chain.length() > 0) chain.append(',');
                chain.append(c.getName());
            }
            rows.add("TYPE\t" + getKey.invoke(entities, type) + "\t" + chain);
        }

        int comps = 0;
        for (Field f : Class.forName(cComponents).getDeclaredFields()) {
            if (!Modifier.isStatic(f.getModifiers())) continue;
            f.setAccessible(true);
            Object v = f.get(null);
            if (v == null) continue;
            Object key = getKey.invoke(components, v);
            if (key == null) continue;
            comps++;
            rows.add("COMP\t" + f.getName() + "\t" + key);
        }

        int enums = 0, regs = 0;
        while (i < args.length) {
            String kind = args[i++];
            if (kind.equals("ENUM")) {
                String label = args[i++], cls = args[i++], mId = args[i++];
                Class<?> e = Class.forName(cls);
                Method values = e.getMethod("values");
                Method ser = null;
                for (Method m : e.getMethods()) {
                    if (m.getName().equals(mSerialized) && m.getParameterCount() == 0
                            && m.getReturnType() == String.class) ser = m;
                }
                Method id = mId.equals("-") ? null : e.getDeclaredMethod(mId);
                if (id != null) id.setAccessible(true);
                for (Object c : (Object[]) values.invoke(null)) {
                    enums++;
                    rows.add("ENUM\t" + label + "\t" + ((Enum<?>) c).name() + "\t"
                            + (ser == null ? "-" : ser.invoke(c)) + "\t"
                            + (id == null ? "-" : id.invoke(c)));
                }
            } else if (kind.equals("REG")) {
                String label = args[i++], field = args[i++];
                Object reg = staticField(builtIn, field);
                for (Object v : (Iterable<?>) reg) {
                    regs++;
                    rows.add("REG\t" + label + "\t" + getKey.invoke(reg, v));
                }
            } else {
                throw new IllegalArgumentException("unknown request " + kind);
            }
        }
        rows.sort(String::compareTo);
        try (PrintWriter w = new PrintWriter(out, StandardCharsets.UTF_8)) {
            for (String r : rows) w.println(r);
        }
        System.out.println("DUMPED types=" + types + " components=" + comps + " enums=" + enums + " registries=" + regs);
    }

    private static Object staticField(Class<?> c, String name) throws Exception {
        Field f = c.getDeclaredField(name);
        f.setAccessible(true);
        return f.get(null);
    }
}
