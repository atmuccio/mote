import java.util.function.Supplier;

public class GardenPlanner {
    public static class TrellisWithStaticSetter {
        public static void setDefaultHeight(String ignored) { }
    }

    public static class ViaReference {
        public static void touch() { }
    }

    public static class NeverCreated {                              // expect: utility-class-constructor
        public static void helper() { }
    }

    private final TrellisWithStaticSetter trellis = new TrellisWithStaticSetter();
    private final Supplier<ViaReference> factory = ViaReference::new;

    // A string or comment mentioning NeverCreated() is not an instantiation, nor renewNeverCreated().
    static void renewNeverCreated() { }
}

class Agent {
    public static void premain(String args) { }

    public static void log(String m) { }
}

class AttachAgent {
    public static void agentmain(String args, java.lang.instrument.Instrumentation inst) { }
}
