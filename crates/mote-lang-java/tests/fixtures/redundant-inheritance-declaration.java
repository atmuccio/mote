import java.io.IOException;
import java.io.Serializable;
import java.util.AbstractList;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.RandomAccess;

class Plain extends Object {                                                     // expect: redundant-inheritance-declaration
}

class Qualified extends java.lang.Object {                                       // expect: redundant-inheritance-declaration
}

class Failure extends RuntimeException implements Serializable {                 // expect: redundant-inheritance-declaration
}

class Names extends ArrayList<String> implements List<String>, RandomAccess {    // expect: redundant-inheritance-declaration, redundant-inheritance-declaration
}

interface Store extends Collection<String>, List<String> {                       // expect: redundant-inheritance-declaration
}

enum Mode implements Comparable<Mode> {                                          // expect: redundant-inheritance-declaration
    ON, OFF
}

interface Task {
    void run() throws IOException;

    String name(int index, String... parts);
}

class Base implements Task {
    public void run() {
    }

    public String name(int index, String... parts) {
        return "";
    }
}

class Derived extends Base implements Task {                                     // expect: redundant-inheritance-declaration
}

abstract class Job implements Task {
    public abstract void run() throws IOException;                               // expect: redundant-inheritance-declaration

    @Override
    public abstract String name(int i, String... p);                             // expect: redundant-inheritance-declaration
}

interface TimedTask extends Task {
    void run() throws IOException;                                               // expect: redundant-inheritance-declaration
}

// Compliant
class Own implements Serializable {
}

class Unknown extends com.acme.Base implements Serializable {                    // hierarchy of another file
}

abstract class Narrower implements Task {
    public abstract void run();                                                  // no longer throws: a new contract

    /** Returns the display name. */
    public abstract String name(int index, String... parts);                     // documented
}

abstract class Annotated implements Task {
    @Deprecated
    public abstract void run() throws IOException;
}

abstract class Reabstracted extends Base implements Task {                       // expect: redundant-inheritance-declaration
    public abstract void run() throws IOException;                               // forces subclasses to implement it again
}

interface Other {
    default void run() throws IOException {
    }
}

abstract class Diamond implements Task, Other {
    public abstract void run() throws IOException;                               // resolves the default of Other
}

abstract class External implements Task, Runnable {
    public abstract void run() throws IOException;
}

interface Generic<T> {
    void accept(T value);
}

abstract class Consumer implements Generic<String> {
    public abstract void accept(String value);                                   // a generic interface
}

class Lists extends AbstractList<String> {
    public String get(int i) {
        return "";
    }

    public int size() {
        return 0;
    }
}

@interface NotNull {
}

interface Api {
    String read(@NotNull String key);

    @NotNull
    String load(String key);
}

interface Api2 extends Api {
    @Override
    String read(String key);                                                    // drops the parameter constraint

    String load(String key);                                                    // drops the method annotation
}
