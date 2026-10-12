import java.util.List;

class Paths {
    boolean positives(String path, List<String> names, StringBuilder sb) {
        boolean a = path.indexOf('/') > 0;                          // expect: index-of-positive-check
        boolean b = 0 < path.indexOf("x");                          // expect: index-of-positive-check
        boolean c = names.indexOf("admin") > 0;                     // expect: index-of-positive-check
        boolean d = sb.indexOf(":") > 0;                            // expect: index-of-positive-check
        boolean e = (path.indexOf('.')) > 0;                        // expect: index-of-positive-check
        return a || b || c || d || e;
    }

    boolean negatives(String path, Finder f) {
        int idx = path.indexOf('/');
        boolean a = idx > 0;
        boolean b = path.indexOf('/') >= 0;
        boolean c = path.indexOf('/') > -1;
        boolean d = path.indexOf('/') != -1;
        boolean e = path.indexOf('/') > 1;
        boolean g = f.indexOf("x") > 0;                             // unknown type
        boolean h = path.lastIndexOf('/') > 0;
        String key = path;
        if (path.indexOf(':') > 0) {                                // a non-empty prefix is required
            key = path.substring(0, path.indexOf(':'));
        }
        String k = path.indexOf('=') > 0 ? path.substring(0, 1) : path;
        boolean m = path.indexOf('#') > 0 && path.charAt(0) != '#';
        return a || b || c || d || e || g || h || m || key.isEmpty() || k.isEmpty();
    }
}
