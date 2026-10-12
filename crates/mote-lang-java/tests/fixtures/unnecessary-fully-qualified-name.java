package com.acme.report;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import org.slf4j.Logger;

class Report {
    java.util.List<String> lines = new java.util.ArrayList<>();     // expect: unnecessary-fully-qualified-name, unnecessary-fully-qualified-name
    java.lang.String title;                                         // expect: unnecessary-fully-qualified-name
    com.acme.report.Section section;                                // expect: unnecessary-fully-qualified-name
    org.slf4j.Logger log;                                           // expect: unnecessary-fully-qualified-name
    java.util.Map.Entry<String, String> entry;                      // expect: unnecessary-fully-qualified-name

    @java.lang.Deprecated                                           // expect: unnecessary-fully-qualified-name
    void render() {
        List<String> x = java.util.Collections.emptyList();         // expect: unnecessary-fully-qualified-name
        Object o = java.util.Locale.ROOT;                           // Locale is not imported
        java.util.Set<String> s = null;                             // Set is not imported
        java.awt.List awt = null;                                   // a different List
        java.sql.Date d = null;                                     // not imported
        String n = java.lang.Integer.toString(1);                   // expect: unnecessary-fully-qualified-name
    }
}

class Section {
}

class Date {
    java.util.Date created;                                         // `Date` here is this file's class
}
