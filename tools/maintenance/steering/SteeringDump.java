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
 * Ask the pinned Minecraft server jar what carries a rider.
 *
 * <p>For every registered entity type it reports the class the type constructs
 * (read off the generic signature of its static field on EntityType, as
 * {@code PeacefulDump} does) and that class's superclass chain; for every
 * registered item that is a food-on-a-stick it reports the entity type the item
 * steers (its {@code canInteractWith} field). The caller turns those facts into
 * the steering table; this dumper decides nothing.
 *
 * <p>Every obfuscated name is passed in, resolved from the official mappings for
 * the same pin — none is written down here.
 *
 * <p>Output: {@code TYPE\t<id>\t<class>,<super>,…} per entity type and
 * {@code ITEM\t<item id>\t<entity id>} per steering item.
 */
public final class SteeringDump {
    public static void main(String[] args) throws Exception {
        int i = 0;
        String out = args[i++];
        String cShared = args[i++], mDetect = args[i++];
        String cBootstrap = args[i++], mBoot = args[i++];
        String cBuiltIn = args[i++], fEntityType = args[i++], fItem = args[i++];
        String cRegistry = args[i++], mGetKey = args[i++];
        String cEntityType = args[i++];
        String cFoodOnAStick = args[i++], fCanInteractWith = args[i++];

        Method detect = Class.forName(cShared).getDeclaredMethod(mDetect);
        detect.setAccessible(true);
        detect.invoke(null);
        Method boot = Class.forName(cBootstrap).getDeclaredMethod(mBoot);
        boot.setAccessible(true);
        boot.invoke(null);

        Class<?> builtIn = Class.forName(cBuiltIn);
        Field reg = builtIn.getDeclaredField(fEntityType);
        reg.setAccessible(true);
        Object entities = reg.get(null);
        Field itemReg = builtIn.getDeclaredField(fItem);
        itemReg.setAccessible(true);
        Object items = itemReg.get(null);
        Method getKey = Class.forName(cRegistry).getMethod(mGetKey, Object.class);
        getKey.setAccessible(true);

        Class<?> entityType = Class.forName(cEntityType);
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
        int types = 0;
        for (Object type : (Iterable<?>) entities) {
            types++;
            Class<?> cls = constructed.get(type);
            if (cls == null) {
                throw new IllegalStateException(
                        "no EntityType field holds the registry entry " + getKey.invoke(entities, type));
            }
            StringBuilder chain = new StringBuilder();
            for (Class<?> c = cls; c != null; c = c.getSuperclass()) {
                if (chain.length() > 0) chain.append(',');
                chain.append(c.getName());
            }
            rows.add("TYPE\t" + getKey.invoke(entities, type) + "\t" + chain);
        }

        Class<?> foodOnAStick = Class.forName(cFoodOnAStick);
        Field steers = foodOnAStick.getDeclaredField(fCanInteractWith);
        steers.setAccessible(true);
        int steering = 0;
        for (Object item : (Iterable<?>) items) {
            if (!foodOnAStick.isInstance(item)) continue;
            steering++;
            rows.add("ITEM\t" + getKey.invoke(items, item) + "\t" + getKey.invoke(entities, steers.get(item)));
        }
        rows.sort(String::compareTo);
        try (PrintWriter w = new PrintWriter(out, StandardCharsets.UTF_8)) {
            for (String r : rows) w.println(r);
        }
        System.out.println("DUMPED types=" + types + " steering=" + steering);
    }
}
