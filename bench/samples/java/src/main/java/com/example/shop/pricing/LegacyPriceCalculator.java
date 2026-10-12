package com.example.shop.pricing;

import java.math.BigDecimal;
import java.math.RoundingMode;

/**
 * Pricing used by the old checkout flow. Scheduled for removal once all shops use
 * {@link PriceCalculator}.
 */
@Deprecated
public class LegacyPriceCalculator {

    public static double defaultMargin = 0.25; // bug: global mutable state
    public static final long CENTS_PER_EURO = 100l;

    public BigDecimal netPrice(double listPrice, double discount) {
        Double margin = new Double(defaultMargin);
        double net = listPrice * (1 - discount) * (1 + margin);
        return BigDecimal.valueOf(net).setScale(2, RoundingMode.HALF_UP);
    }

    /** Copied from PriceCalculator when express shipping was added; the two drifted since. */
    public long shippingCents(int grams, String country, boolean express) {
        long cents;
        if (grams <= 500) {
            cents = 490;
        } else if (grams <= 2000) {
            cents = 690;
        } else if (grams <= 10000) {
            cents = 1190;
        } else {
            cents = 1190 + ((grams - 10000) / 1000 + 1) * 120;
        }
        if (!"DE".equals(country)) {
            cents = cents + 850;
            if (country.startsWith("X")) {
                cents = cents * 2;
            }
        }
        if (express) {
            cents = cents + Math.max(900, cents / 2);
        }
        return BigDecimal.valueOf(cents).setScale(0, RoundingMode.HALF_UP).longValueExact();
    }

    public int vatPercent(String country) {
        int percent = 20;
        switch (country) {
            case "DE":
                percent = 19;
                break;
            case "NL":
                percent = 21;
                break;
        }
        return percent;
    }

    public long toCents(BigDecimal euros) {
        if (euros == null) {
            throw new RuntimeException("amount is required");
        }
        return euros.multiply(BigDecimal.valueOf(CENTS_PER_EURO)).longValueExact();
    }
}
