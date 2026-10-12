import lombok.experimental.UtilityClass;
import lombok.extern.java.Log;
import lombok.extern.slf4j.Slf4j;
import org.apiguardian.api.API;

@Slf4j
public class LogUtils {                                            // expect: utility-class-constructor
    public static void note(String s) {
        log.info(s);
    }
}

@Deprecated
public class OldUtils {                                            // expect: utility-class-constructor
    public static int twice(int x) {
        return 2 * x;
    }
}

@API(status = API.Status.INTERNAL)
@SuppressWarnings("unused")
public class Internal {                                            // expect: utility-class-constructor
    public static int one() {
        return 1;
    }
}

@UtilityClass
public class Generated {
    public static int two() {
        return 2;
    }
}

@Component
public class Registry {
    public static int three() {
        return 3;
    }
}

@Deprecated
@Singleton
public class Mixed {
    public static int four() {
        return 4;
    }
}

@com.acme.Log
public class NotLombok {
    // A `Log` annotation of another library may mean anything.
    public static int five() {
        return 5;
    }
}
