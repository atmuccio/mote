// mote-test: path app/src/main/java/com/acme/report/Loader.java
package com.acme.report;

import static org.assertj.core.api.Assertions.*;
import static org.assertj.core.util.Lists.*;
import static org.testng.Assert.*;

class Loader {
    void load(Object source) {
        assertThat(source).isNotNull();                              // expect: test-assertion-in-production
        assertNotNull(source);                                       // expect: test-assertion-in-production
    }
}
