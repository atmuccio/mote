import java.sql.SQLException;

class Dispatcher {
    void handle(Object event, Class<?> type, String name, SQLException ex) {
        if (event.getClass().getSimpleName().equals("OrderCreated")) { }        // expect: class-name-string-comparison
        if ("Dispatcher".equals(getClass().getSimpleName())) { }                 // expect: class-name-string-comparison
        if (ex.getClass().getName().equals("SQLTimeoutException")) { }           // expect: class-name-string-comparison
        if ("Order".equals(event.getClass().getCanonicalName())) { }             // expect: class-name-string-comparison

        // Compliant
        if (event instanceof String) { }
        if (event.getClass() == String.class) { }
        if (event.getClass().getName().equals("org.hsqldb.types.Binary")) { }   // optional class, no dependency
        if (event.getClass().getName().startsWith("java.")) { }
        if (event.getClass().getName().equals(name)) { }
        if (type.getName().equals("Order")) { }
        if (String.class.getName().equals(name)) { }
        if (event.getClass().getSimpleName().equalsIgnoreCase(name)) { }
    }
}
