// mote-test: path src/main/java/com/acme/users/Registry.java
package com.acme.users;

import static org.junit.Assert.assertNotNull;
import static org.assertj.core.api.Assertions.assertThat;

import org.junit.jupiter.api.Assertions;

class Registry {
    void register(Object user) {
        assertNotNull(user);                                         // expect: test-assertion-in-production
        assertThat(user).isNotNull();                                // expect: test-assertion-in-production
        Assertions.assertTrue(user != null);                         // expect: test-assertion-in-production
        check(user);
        Guard.assertTrue(user != null);
    }

    void check(Object o) {
        assertValid(o);
    }

    void assertValid(Object o) {}
}

class Guard {
    static void assertTrue(boolean b) {}
}

class RegistryAssert {
    void verify(Object o) {
        assertNotNull(o);
    }
}
