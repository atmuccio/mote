import java.util.List;

class UniqueNames {
    String unique(String base, List<String> taken) {
        String name = base;
        int suffix = 0;
        boolean done = false;
        while (!done) {
            String before = name;
            for (String t : taken) {
                if (name.equals(t)) {
                    String tail = "_" + suffix;
                    if (suffix == 0) {
                        name = name + tail;
                    } else {
                        name = name.substring(0, name.length() - 2) + tail;
                    }
                    suffix++;
                }
            }
            done = name.equals(before);
        }
        return name;
    }

    String secondPass(List<String> parts) {
        String out = "";
        int j = 0;
        for (String p : parts) {
            if (j == 0) {
                out += "[" + p;
            } else if (j == 1) {
                out += "," + p;
            } else {
                out += p.charAt(0);                     // expect: string-concat-in-loop
            }
            j++;
        }
        return out;
    }

    String noIncrement(List<String> parts, int count) {
        String out = "";
        for (String p : parts) {
            if (count == 0) {
                out = out + p;                          // expect: string-concat-in-loop
            }
        }
        return out;
    }

    String elseBranch(List<String> parts) {
        String out = "";
        int i = 0;
        for (String p : parts) {
            if (i == 0) {
                out = p;
            } else {
                out += "," + p;                         // expect: string-concat-in-loop
            }
            i++;
        }
        return out;
    }
}
