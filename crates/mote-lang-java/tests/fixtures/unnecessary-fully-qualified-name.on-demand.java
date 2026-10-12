package org.acme.connection;

import com.vendor.client.*;
import java.util.List;

// `ConnectionFactory` may also be a class of org.acme.connection declared in another file,
// which would win over the on-demand import: keep the qualified name.
class Factories {
    com.vendor.client.ConnectionFactory vendor;
    java.util.List<String> names;                                   // expect: unnecessary-fully-qualified-name
    org.acme.connection.Pool pool;                                  // the resolver cannot confirm `Pool` here: silent
}
