import org.junit.Rule;
import org.junit.rules.TemporaryFolder;
import com.beust.jcommander.Parameter;

public class Range {
    public int low;                                          // expect: public-instance-field
    public int high, step;                                   // expect: public-instance-field, public-instance-field
    public final int size = 0;
    public static int counter;
    protected int prot;
    int pkg;
    private int priv;
    @Rule public TemporaryFolder folder = new TemporaryFolder();
    @Parameter(names = "-v") public boolean verbose;

    public static class Nested {
        public String name;                                  // expect: public-instance-field
    }

    private static class Node {
        public Node next;                                    // effectively private
    }

    enum Kind {
        A;
        public int weight;                                   // enums are not checked
    }

    Object anon = new Object() {
        public int anonField;
    };

    void m() {
        class Local {
            public int localField;
        }
    }
}

class PackagePrivate {
    public String exposed;                                   // expect: public-instance-field
}

interface Api {
    int X = 1;
}
