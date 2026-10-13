// mote-test: params java_release=23
package com.acme.demo;

import java.io.IOException;
import java.time.Duration;
import java.util.Deque;
import java.util.List;
import java.util.Map;
import java.util.Optional;                        // expect: unused-import
import java.util.Queue;
import java.util.Set;
import java.util.SortedMap;                       // expect: unused-import
import java.util.SortedSet;                       // expect: unused-import
import java.util.concurrent.Callable;
import java.util.function.Function;
import java.util.function.Supplier;

/// Holds a [List] of values and maps them with a [Function].
///
/// Accepts [the queue][Queue] and a [Deque] through [#accept(Set, Duration)].
/// Also [Callable#call()] and {@link Supplier}.
/// See the [project page](https://example.com/Optional) for [Optional details](#Optional).
///
/// @see Map
/// @throws IOException when it fails
//// A banner comment mentioning [SortedMap] is not documentation.
// A plain comment mentioning [SortedSet] is not documentation either.
class UnusedImportsMarkdown {
    void accept(Object a, Object b) {
    }
}
