class BoxingRoundtripShadowed {
    static class Integer {
        static Integer valueOf(String s) { return new Integer(); }
        int intValue() { return 0; }
    }

    int shadowed(String text) {
        return Integer.valueOf(text).intValue();                             // user class named Integer
    }
}
