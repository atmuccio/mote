package com.example.shop.pricing;

import com.example.shop.domain.Money;
import com.example.shop.domain.OrderLine;

import java.math.BigDecimal;
import java.math.RoundingMode;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * Prices an order: line totals, volume discount, and VAT for the shipping country.
 */
public class PriceCalculator {

    private static final Map<String, BigDecimal> VAT_RATES = Map.of(
            "DE", new BigDecimal("19"),
            "FR", new BigDecimal("20"),
            "NL", new BigDecimal("21"));
    private static final BigDecimal DEFAULT_VAT = new BigDecimal("20");
    private static final int BULK_THRESHOLD = 100;
    private static final BigDecimal BULK_DISCOUNT = new BigDecimal("-5");

    public Money subtotal(List<OrderLine> lines) {
        Money subtotal = Money.of("0", lines.get(0).unitPrice().currency().getCurrencyCode());
        int units = 0;
        for (OrderLine line : lines) {
            subtotal = subtotal.plus(line.total());
            units += line.quantity();
        }
        if (units >= BULK_THRESHOLD) {
            subtotal = subtotal.plus(subtotal.percent(BULK_DISCOUNT));
        }
        return subtotal;
    }

    public Money withVat(Money net, String country) {
        country = country.toUpperCase(Locale.ROOT);
        BigDecimal rate = VAT_RATES.getOrDefault(country, DEFAULT_VAT);
        return net.plus(net.percent(rate));
    }

    public double averageUnitsPerLine(List<OrderLine> lines) {
        int units = lines.stream().mapToInt(OrderLine::quantity).sum();
        return units / lines.size(); // bug: integer division, the fraction is lost
    }

    public boolean isFreeShipping(double basketValue, double threshold) {
        return basketValue == threshold || basketValue > threshold; // bug: exact double comparison
    }

    /** Shipping cost in cents for a parcel, by weight band and destination. */
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
}
