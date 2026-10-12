import java.util.List;
import java.util.Map;

class Concat {
    private String log = "";
    private StringBuilder sb = new StringBuilder();

    String positives(List<String> fields, Map<String, String> map, int n) {
        String csv = "";
        for (String field : fields) {
            csv += field + ",";                         // expect: string-concat-in-loop
        }
        String out = "";
        int i = 0;
        while (i < n) {
            out = out + i;                              // expect: string-concat-in-loop
            i++;
        }
        String rev = "";
        for (int j = 0; j < n; j++) {
            rev = j + rev;                              // expect: string-concat-in-loop
        }
        do {
            log += "x";                                 // expect: string-concat-in-loop
            log += "z";                                 // same variable, same loop: reported once
        } while (log.length() < 10);
        while (log.length() < 20) {
            this.log = this.log + "y";                  // expect: string-concat-in-loop
        }
        for (Map.Entry<String, String> e : map.entrySet()) {
            if (e.getKey() != null) {
                csv = csv + e.getKey() + "=" + e.getValue();   // expect: string-concat-in-loop
            }
        }
        return csv + out + rev;
    }

    String negatives(List<String> fields, List<Integer> numbers, int n, Unknown u) {
        for (String field : fields) {
            field = "[" + field + "]";                  // the loop variable: fresh each iteration
            field += "!";
        }
        for (String field : fields) {
            String line = "> ";
            line += field;                              // declared inside the loop body
            System.out.println(line);
        }
        String result = "";
        for (String field : fields) {
            if (field.isEmpty()) {
                result += field;                        // followed by break: runs once
                break;
            }
        }
        String header = "";
        header += "title";                              // not in a loop
        int total = 0;
        for (int k = 0; k < n; k++) {
            total += k;                                 // not a String
        }
        for (String field : fields) {
            sb.append(field);
        }
        for (Object o : u.items()) {
            u.text += o;                                // unknown type
        }
        fields.forEach(f -> {
            String s = "";
            s += f;
        });
        String acc = "";
        for (String field : fields) {
            Runnable r = () -> System.out.println(field);
            acc = field;                                // plain assignment
        }
        String each = "";
        for (String a = ""; a.length() < 3; a += "x") { // loop header, not the body
            each = a;
        }
        return result + header + total + acc + each;
    }
}
