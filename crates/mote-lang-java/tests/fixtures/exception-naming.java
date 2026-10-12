import java.io.IOException;

class InvalidOrder extends RuntimeException { }                   // expect: exception-naming
class Timeout extends IOException { }                             // expect: exception-naming
class Failure extends java.lang.Exception { }                     // expect: exception-naming
class Boom extends Throwable { }                                  // expect: exception-naming
class Fatal extends Error { }                                     // expect: exception-naming
class Broken extends AssertionError { }                           // expect: exception-naming
class Remote extends com.acme.RemoteException { }                 // expect: exception-naming
class Base extends Exception { }                                  // expect: exception-naming
class Derived extends Base { }                                    // expect: exception-naming

class Outer {
    static class Inner extends IllegalStateException { }          // expect: exception-naming
}

// negatives
class InvalidOrderException extends RuntimeException { }
class FatalError extends Error { }
class ValidationError extends ObjectError { }                     // name ends in Error anyway
class Holder extends ObjectError { }                              // unknown `…Error` supertype: may be a data class
class Plain extends Object { }
class Handler extends ExceptionHandler { }                        // not an exception
class Listener implements ExceptionListener { }
class Mapper<T extends Exception> { }
record Failed(Exception cause) { }
interface ProblemException { }
