package dw;

import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collection;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

/**
 * Ask the pinned Minecraft client which boxes each entity model layer builds.
 *
 * <p>Calls the client's own {@code LayerDefinitions.createRoots()} -- the map every
 * {@code ModelLayers} entry is baked from -- and writes, for each layer field
 * named on the command line, the {@code LayerDefinition} it maps to: the texture
 * size and the whole part tree, every part's pose and every cube's
 * {@code texOffs}, origin, dimensions, grow and mirror flag. The values are the
 * objects the game builds, read by reflection, so nothing here is inferred from
 * bytecode and nothing is typed.
 *
 * <p>Every obfuscated name is resolved from the official ProGuard mappings
 * handed in as the first argument; none is written down here, so a version bump
 * is a pin edit and nothing else. Object fields are written under their
 * deobfuscated names, maps with sorted keys, so the dump is a pure function of
 * the jar.
 *
 * <p>Usage: {@code ModelPartDump <mappings> <out.json> <ModelLayers field>...}
 */
public final class ModelPartDump {
    private static final Map<String, String> CLASS_OBF = new HashMap<>();
    private static final Map<String, String> CLASS_DEOBF = new HashMap<>();
    private static final Map<String, Map<String, String>> FIELD_DEOBF = new HashMap<>();
    private static final Map<String, Map<String, String>> METHOD_OBF = new HashMap<>();
    private static final Map<String, Map<String, String>> FIELD_OBF = new HashMap<>();

    public static void main(String[] args) throws Exception {
        readMappings(Path.of(args[0]));
        String out = args[1];

        // Some layer builders read registries (a block's item, a game event), so
        // the registries are booted first, exactly as the client boots them.
        Class<?> shared = cls("net.minecraft.SharedConstants");
        Method detect = shared.getDeclaredMethod(method(shared, "tryDetectVersion"));
        detect.setAccessible(true);
        detect.invoke(null);
        Class<?> bootstrap = cls("net.minecraft.server.Bootstrap");
        Method boot = bootstrap.getDeclaredMethod(method(bootstrap, "bootStrap"));
        boot.setAccessible(true);
        boot.invoke(null);

        Class<?> roots = cls("net.minecraft.client.model.geom.LayerDefinitions");
        Method createRoots = roots.getDeclaredMethod(method(roots, "createRoots"));
        createRoots.setAccessible(true);
        Map<?, ?> all = (Map<?, ?>) createRoots.invoke(null);

        Class<?> layers = cls("net.minecraft.client.model.geom.ModelLayers");
        StringBuilder sb = new StringBuilder();
        sb.append("{\"layers\":{");
        for (int i = 2; i < args.length; i++) {
            String name = args[i];
            Field f = layers.getDeclaredField(FIELD_OBF.get(layers.getName()).get(name));
            f.setAccessible(true);
            Object loc = f.get(null);
            Object def = all.get(loc);
            if (def == null) {
                throw new IllegalStateException("createRoots binds nothing to ModelLayers." + name);
            }
            if (i > 2) sb.append(',');
            str(sb, name);
            sb.append(':');
            dump(sb, def);
        }
        sb.append("},\"roots\":").append(all.size());
        sb.append(",\"player_eye_height\":").append(Float.toString(playerEyeHeight()));
        sb.append(",\"mannequin\":");
        mannequin(sb);
        sb.append('}');
        try (PrintWriter w = new PrintWriter(out, StandardCharsets.UTF_8)) {
            w.println(sb);
        }
        System.out.println("DUMPED layers=" + (args.length - 2) + " roots=" + all.size());
    }

    /**
     * What a mannequin's {@code hidden_layers} field is: its NBT key, the byte a
     * mannequin starts from, and -- by running the field's own codec -- which
     * layer bits each single-id list clears. Encoding {@code ALL_LAYERS} is the
     * list a mannequin that hides nothing writes.
     */
    private static void mannequin(StringBuilder sb) throws Exception {
        Class<?> m = cls("net.minecraft.world.entity.decoration.Mannequin");
        Map<String, String> obf = FIELD_OBF.get(m.getName());
        Field key = m.getDeclaredField(obf.get("HIDDEN_LAYERS_FIELD"));
        key.setAccessible(true);
        Field all = m.getDeclaredField(obf.get("ALL_LAYERS"));
        all.setAccessible(true);
        Field codecField = m.getDeclaredField(obf.get("LAYERS_CODEC"));
        codecField.setAccessible(true);
        @SuppressWarnings("unchecked")
        com.mojang.serialization.Codec<Object> codec =
                (com.mojang.serialization.Codec<Object>) codecField.get(null);
        byte allLayers = all.getByte(null);
        sb.append("{\"field\":");
        str(sb, (String) key.get(null));
        sb.append(",\"all_layers\":").append(allLayers & 0xff);
        Object empty = codec.encodeStart(com.mojang.serialization.JsonOps.INSTANCE, allLayers)
                .getOrThrow();
        sb.append(",\"encode_all_layers\":");
        str(sb, empty.toString());
        sb.append(",\"parts\":[");
        Class<?> part = cls("net.minecraft.world.entity.player.PlayerModelPart");
        Method getId = part.getDeclaredMethod(method(part, "getId"));
        Method getMask = part.getDeclaredMethod(method(part, "getMask"));
        Method serialized = part.getDeclaredMethod(method(part, "getSerializedName"));
        Object[] values = part.getEnumConstants();
        for (int i = 0; i < values.length; i++) {
            Object v = values[i];
            String id = (String) getId.invoke(v);
            com.google.gson.JsonArray one = new com.google.gson.JsonArray();
            one.add((String) serialized.invoke(v));
            Object decoded = codec.parse(com.mojang.serialization.JsonOps.INSTANCE, one).getOrThrow();
            if (i > 0) sb.append(',');
            sb.append("{\"constant\":");
            dump(sb, v);
            sb.append(",\"id\":");
            str(sb, id);
            sb.append(",\"serialized\":");
            str(sb, (String) serialized.invoke(v));
            sb.append(",\"mask\":").append(getMask.invoke(v));
            sb.append(",\"decoded_alone\":").append(((Number) decoded).intValue() & 0xff);
            sb.append('}');
        }
        sb.append("]}");
    }

    /** The eye height of a standing player, in blocks: {@code EntityType.PLAYER}'s dimensions. */
    private static float playerEyeHeight() throws Exception {
        Class<?> types = cls("net.minecraft.world.entity.EntityType");
        Field player = types.getDeclaredField(FIELD_OBF.get(types.getName()).get("PLAYER"));
        player.setAccessible(true);
        Field dims = types.getDeclaredField(FIELD_OBF.get(types.getName()).get("dimensions"));
        dims.setAccessible(true);
        Object d = dims.get(player.get(null));
        Class<?> dc = cls("net.minecraft.world.entity.EntityDimensions");
        Field eye = dc.getDeclaredField(FIELD_OBF.get(dc.getName()).get("eyeHeight"));
        eye.setAccessible(true);
        return eye.getFloat(d);
    }

    private static Class<?> cls(String deobf) throws ClassNotFoundException {
        String obf = CLASS_OBF.get(deobf);
        if (obf == null) throw new IllegalStateException("no mapping for " + deobf);
        return Class.forName(obf);
    }

    private static String method(Class<?> c, String name) {
        String obf = METHOD_OBF.getOrDefault(c.getName(), Map.of()).get(name);
        if (obf == null) throw new IllegalStateException("no zero-argument mapping for " + name);
        return obf;
    }

    private static void readMappings(Path p) throws Exception {
        String cur = null;
        for (String line : Files.readAllLines(p, StandardCharsets.UTF_8)) {
            if (line.startsWith("#") || line.isBlank()) continue;
            if (!line.startsWith(" ")) {
                String[] parts = line.substring(0, line.length() - 1).split(" -> ");
                CLASS_OBF.put(parts[0], parts[1]);
                CLASS_DEOBF.put(parts[1], parts[0]);
                cur = parts[1];
                FIELD_DEOBF.put(cur, new HashMap<>());
                FIELD_OBF.put(cur, new HashMap<>());
                METHOD_OBF.put(cur, new HashMap<>());
                continue;
            }
            String t = line.trim();
            if (t.startsWith("#")) continue;
            String[] arrow = t.split(" -> ");
            if (arrow.length != 2) continue;
            String lhs = arrow[0];
            String obf = arrow[1];
            if (lhs.contains("(")) {
                // `a:b:<ret> <name>(<args>)` -- only zero-argument methods are named.
                if (lhs.endsWith("()")) {
                    String sig = lhs.substring(0, lhs.length() - 2);
                    String name = sig.substring(sig.lastIndexOf(' ') + 1);
                    METHOD_OBF.get(cur).putIfAbsent(name, obf);
                }
            } else {
                String name = lhs.substring(lhs.lastIndexOf(' ') + 1);
                FIELD_DEOBF.get(cur).put(obf, name);
                FIELD_OBF.get(cur).put(name, obf);
            }
        }
    }

    private static void dump(StringBuilder sb, Object o) throws Exception {
        if (o == null) {
            sb.append("null");
        } else if (o instanceof Boolean || o instanceof Integer || o instanceof Long) {
            sb.append(o);
        } else if (o instanceof Float || o instanceof Double) {
            double d = ((Number) o).doubleValue();
            if (Double.isNaN(d) || Double.isInfinite(d)) {
                str(sb, o.toString());
            } else {
                sb.append(Float.toString(((Number) o).floatValue()));
            }
        } else if (o instanceof String) {
            str(sb, (String) o);
        } else if (o instanceof Enum<?>) {
            Enum<?> e = (Enum<?>) o;
            String owner = e.getDeclaringClass().getName();
            String name = FIELD_DEOBF.getOrDefault(owner, Map.of()).getOrDefault(e.name(), e.name());
            str(sb, name);
        } else if (o instanceof Map<?, ?>) {
            TreeMap<String, Object> sorted = new TreeMap<>();
            for (Map.Entry<?, ?> e : ((Map<?, ?>) o).entrySet()) {
                sorted.put(String.valueOf(e.getKey()), e.getValue());
            }
            sb.append('{');
            boolean first = true;
            for (Map.Entry<String, Object> e : sorted.entrySet()) {
                if (!first) sb.append(',');
                first = false;
                str(sb, e.getKey());
                sb.append(':');
                dump(sb, e.getValue());
            }
            sb.append('}');
        } else if (o instanceof Collection<?>) {
            List<String> items = new ArrayList<>();
            for (Object x : (Collection<?>) o) {
                StringBuilder one = new StringBuilder();
                dump(one, x);
                items.add(one.toString());
            }
            // A set's iteration order is the hash's; a list's is the builder's.
            if (o instanceof java.util.Set<?>) items.sort(String::compareTo);
            sb.append('[').append(String.join(",", items)).append(']');
        } else if (o.getClass().getName().startsWith("org.joml.")) {
            sb.append('{');
            String[] axes = {"x", "y", "z"};
            for (int i = 0; i < axes.length; i++) {
                Field f = o.getClass().getField(axes[i]);
                if (i > 0) sb.append(',');
                str(sb, axes[i]);
                sb.append(':');
                dump(sb, f.get(o));
            }
            sb.append('}');
        } else if (CLASS_DEOBF.containsKey(o.getClass().getName())) {
            sb.append('{');
            str(sb, "$class");
            sb.append(':');
            str(sb, CLASS_DEOBF.get(o.getClass().getName()));
            TreeMap<String, Object> fields = new TreeMap<>();
            for (Class<?> c = o.getClass(); c != null && CLASS_DEOBF.containsKey(c.getName()); c = c.getSuperclass()) {
                for (Field f : c.getDeclaredFields()) {
                    if (Modifier.isStatic(f.getModifiers())) continue;
                    f.setAccessible(true);
                    String name = FIELD_DEOBF.get(c.getName()).getOrDefault(f.getName(), f.getName());
                    fields.putIfAbsent(name, f.get(o));
                }
            }
            for (Map.Entry<String, Object> e : fields.entrySet()) {
                sb.append(',');
                str(sb, e.getKey());
                sb.append(':');
                dump(sb, e.getValue());
            }
            sb.append('}');
        } else {
            str(sb, o.toString());
        }
    }

    private static void str(StringBuilder sb, String s) {
        sb.append('"');
        for (char c : s.toCharArray()) {
            if (c == '"' || c == '\\') sb.append('\\').append(c);
            else if (c < 0x20) sb.append(String.format("\\u%04x", (int) c));
            else sb.append(c);
        }
        sb.append('"');
    }
}
