import java.lang.reflect.InvocationTargetException;
import javax.management.JMException;
import javax.management.MBeanException;
import javax.management.RuntimeMBeanException;

class Dumper {
    void copy(Object target, Object source) {
        try {
            copyProperties(target, source);
        } catch (IllegalAccessException e) {
            throw new RuntimeException(e);
        } catch (InvocationTargetException e) {                     // expect: identical-catch-branches
            throw new RuntimeException(e);
        }
    }

    void dump() throws Exception {
        try {
            invoke();
        } catch (RuntimeMBeanException e) {
            throw unwrap(e);
        } catch (MBeanException e) {                                // expect: identical-catch-branches
            throw unwrap(e);
        }
        try {
            invoke();
        } catch (MBeanException e) {
            throw unwrap(e);
        } catch (JMException e) {                                   // supertype: cannot merge
            throw unwrap(e);
        }
        try {
            invoke();
        } catch (InvocationTargetException e) {
            throw unwrap(e);
        } catch (ReflectiveOperationException e) {                  // supertype: cannot merge
            throw unwrap(e);
        }
    }

    Exception unwrap(Exception e) { return e; }
    void invoke() throws Exception { }
    void copyProperties(Object a, Object b) throws IllegalAccessException, InvocationTargetException { }
}
