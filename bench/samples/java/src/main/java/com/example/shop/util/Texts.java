package com.example.shop.util;

import java.text.Normalizer;
import java.util.Locale;
import java.util.regex.Pattern;

/**
 * Small text helpers shared by the web and export layers.
 */
public class Texts {

    private static final Pattern NON_SLUG = Pattern.compile("[^a-z0-9]+");
    private static final Pattern DIACRITICS = Pattern.compile("\\p{M}+");

    /** "Crème Brûlée 250 g" becomes "creme-brulee-250-g". */
    public static String slug(String text) {
        String ascii = DIACRITICS.matcher(Normalizer.normalize(text, Normalizer.Form.NFD)).replaceAll("");
        String slug = NON_SLUG.matcher(ascii.toLowerCase(Locale.ROOT)).replaceAll("-");
        return slug.replaceAll("^-|-$", "");
    }

    public static String truncate(String text, int maxLength) {
        if (text.length() <= maxLength) {
            return text;
        }
        return text.substring(0, Math.max(0, maxLength - 1)) + "…";
    }
}
