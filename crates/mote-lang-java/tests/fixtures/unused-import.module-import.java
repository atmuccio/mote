import module java.base;
import module java.sql;
import java.util.List;                                              // expect: unused-import
import java.util.Map;

// Module imports (Java 25) bring in every package a module exports: they are never reported
// as unused, and the single-type imports next to them are checked as usual.
class ModuleImports {
    Map<String, Object> values;
}
