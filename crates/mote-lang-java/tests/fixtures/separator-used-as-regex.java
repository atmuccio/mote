import java.io.File;
import java.util.regex.Pattern;

class Paths2 {
    String[] wrong(String version, String path, String line) {
        String[] a = version.split(".");                            // expect: separator-used-as-regex
        String[] b = line.split("|");                               // expect: separator-used-as-regex
        String[] c = path.split(File.separator);                    // expect: separator-used-as-regex
        String d = path.replaceAll(File.separator, "/");            // expect: separator-used-as-regex
        Pattern p = Pattern.compile(File.separator);                // expect: separator-used-as-regex
        boolean m = path.matches(java.io.File.separator);           // expect: separator-used-as-regex
        String[] e = "a.b".split(".", 2);                           // expect: separator-used-as-regex
        return a;
    }

    String[] fine(String version, String path, Splitter splitter, String line) {
        String[] a = version.split("\.");
        String[] b = line.split("\|");
        String[] c = path.split(Pattern.quote(File.separator));
        String d = path.replace(File.separator, "/");
        String masked = line.replaceAll(".", "*");
        String[] e = splitter.split(".");
        String[] f = line.split(",");
        String[] g = line.split("[.]");
        String sep = File.separator + "x";
        return a;
    }

    interface Splitter { String[] split(String s); }
}
