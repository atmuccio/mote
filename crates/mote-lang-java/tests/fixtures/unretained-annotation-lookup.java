import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.reflect.Method;

class Plugins {
    @interface Audited { }

    @Retention(RetentionPolicy.CLASS)
    @interface Tracked { }

    @Retention(RetentionPolicy.RUNTIME)
    @interface Exposed { }

    @java.lang.annotation.Retention(value = RetentionPolicy.RUNTIME)
    @interface Named { }

    boolean wrong(Method m, Class<?> type) {
        boolean a = m.isAnnotationPresent(Audited.class);              // expect: unretained-annotation-lookup
        Tracked t = type.getAnnotation(Tracked.class);                 // expect: unretained-annotation-lookup
        boolean o = m.isAnnotationPresent(Override.class);             // expect: unretained-annotation-lookup
        Object s = m.getDeclaredAnnotation(SuppressWarnings.class);    // expect: unretained-annotation-lookup
        return a || o;
    }

    boolean fine(Method m, Class<?> type) {
        boolean a = m.isAnnotationPresent(Exposed.class);
        Named n = type.getAnnotation(Named.class);
        boolean d = m.isAnnotationPresent(Deprecated.class);
        boolean x = m.isAnnotationPresent(Imported.class);
        boolean f = m.isAnnotationPresent(FunctionalInterface.class);
        return a || d || x || f;
    }
}
