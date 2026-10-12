// mote-test: path src/main/java/demo/Parser.java
// mote-test: params allow_package_private=true
package demo;

// Without a public type, the one named after the file is kept and the others reported.
class Helper {                                                // expect: multiple-top-level-types
}

class Parser {
}
