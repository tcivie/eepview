package io.github.tcivie.eepview.router;

import java.util.HashMap;
import java.util.Map;

final class Args {
    private final Map<String, String> values;

    private Args(Map<String, String> values) {
        this.values = values;
    }

    static Args parse(String[] args) {
        Map<String, String> map = new HashMap<>();
        for (String arg : args == null ? new String[0] : args) {
            putPair(map, arg);
        }
        return new Args(map);
    }

    private static void putPair(Map<String, String> map, String arg) {
        int eq = arg.indexOf('=');
        if (eq <= 0) {
            throw new IllegalArgumentException("expected key=value, got: " + arg);
        }
        map.put(arg.substring(0, eq), arg.substring(eq + 1));
    }

    String require(String key) {
        String value = values.get(key);
        if (value == null || value.isEmpty()) {
            throw new IllegalArgumentException("missing argument: " + key + "=...");
        }
        return value;
    }

    String get(String key, String fallback) {
        return values.getOrDefault(key, fallback);
    }
}
