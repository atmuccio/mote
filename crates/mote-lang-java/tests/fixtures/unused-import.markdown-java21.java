// mote-test: params java_release=21
// A project compiled for Java 21 may be documented with `///` runs (the JDK running javadoc
// decides): their links count as uses on any release.
package com.acme.demo;

import java.util.List;
import java.util.Map;
import java.util.Optional;                        // expect: unused-import
import java.util.Set;

/// Holds a [List] of values keyed as in [the map][Map].
/** Javadoc references count on every release: {@link Set}. */
class UnusedImportsMarkdown {
}
