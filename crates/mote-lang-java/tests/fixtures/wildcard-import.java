package demo;

import java.util.*;                                           // expect: wildcard-import
import java.util.List;
import static java.util.Objects.*;                            // expect: wildcard-import
import static java.util.Objects.requireNonNull;
import java.util.Map.Entry;
import java.util.concurrent.atomic.*;                         // expect: wildcard-import

class WildcardImport {
    List<String> names = new ArrayList<>();
    // A comment mentioning import java.io.*; is not an import.
    String text = "import java.io.*;";
}
