// Java 25: module import declarations (JEP 511), next to ordinary imports.
import module java.base;
import module java.sql;
import java.util.List;
import static java.util.Objects.requireNonNull;

class ModuleImports {
    List<String> names = new ArrayList<>();
    Connection connection;

    ModuleImports(Connection connection) {
        this.connection = requireNonNull(connection);
    }
}
