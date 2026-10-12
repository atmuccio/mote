package com.example.shop.domain;

/**
 * One line of an order: a product, how many, and the unit price at the time of ordering.
 */
public record OrderLine(String sku, int quantity, Money unitPrice) {

    public OrderLine {
        if (sku == null || sku.isBlank()) {
            throw new IllegalArgumentException("sku must not be blank");
        }
        if (quantity <= 0) {
            throw new IllegalArgumentException("quantity must be positive: " + quantity);
        }
    }

    public Money total() {
        return unitPrice.times(quantity);
    }
}
