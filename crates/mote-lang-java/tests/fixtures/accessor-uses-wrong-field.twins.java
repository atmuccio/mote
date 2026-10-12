class Settings {
    private boolean cachingEnabled;
    private boolean cacheingEnabled;
    private String colour;
    private String color;

    public boolean isCachingEnabled() { return cachingEnabled; }

    public void setCachingEnabled(boolean enabled) { this.cachingEnabled = enabled; }

    // A misspelled alias kept for compatibility: its setter writes both fields, so the getter
    // returning the real field is deliberate.
    public boolean isCacheingEnabled() { return this.cachingEnabled; }

    /** @deprecated use {@link #setCachingEnabled(boolean)} */
    @Deprecated
    public void setCacheingEnabled(boolean enabled) {
        this.cachingEnabled = enabled;
        this.cacheingEnabled = enabled;
    }

    public String getColor() { return color; }

    // The setter of this property writes only one field: no alias, still a twin of getColor.
    public String getColour() { return color; }                    // expect: accessor-uses-wrong-field

    public void setColour(String c) {
        color = c;
        notifyChange();
    }

    void notifyChange() { }
}

class Frame {
    private int width;
    private int height;
    private int left;
    private int right;

    // Swapped pair: each getter returns the other's field.
    public int getWidth() { return height; }                       // expect: accessor-uses-wrong-field

    public int getHeight() { return width; }                       // expect: accessor-uses-wrong-field

    // The accessor named after the used field does something else: no copy-paste twin.
    public int getLeft() { return right; }

    public int getRight() { return compute(); }

    int compute() { return left + 1; }
}
