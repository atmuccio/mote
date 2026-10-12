// mote-test: params max_lambda_lines=3 max_anonymous_class_lines=5 max_nested_class_lines=5
import java.util.List;

class Orders {
    void process(List<String> orders) {
        orders.forEach(order -> {                                       // expect: nested-code-too-long
            String a = order.trim();
            String b = a.toUpperCase();
            System.out.println(b);
        });
        orders.forEach(order -> {
            // comments and blank lines do not count

            System.out.println(order);
            /* nor do
               block comments */
        });
        orders.forEach(o -> System.out.println(o));
        Runnable r = new Runnable() {                                   // expect: nested-code-too-long
            @Override
            public void run() {
                System.out.println("a");
                System.out.println("b");
            }
        };
        Runnable s = new Runnable() {
            public void run() {
                System.out.println("short");
            }
        };
        Object o = new Object();
    }

    static class Inner {                                                // expect: nested-code-too-long
        int a;
        int b;
        int c;
        int d;
        int e;
    }

    static class Small {
        int a;
    }

    enum Mode {
        ON, OFF
    }

    void local() {
        class Helper {                                                  // expect: nested-code-too-long
            int a;
            int b;
            int c;
            String text = """
                one
                two
                """;
        }
    }
}

class TopLevelIsNotMeasured {
    int a;
    int b;
    int c;
    int d;
    int e;
    int f;
}
