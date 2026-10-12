package com.example.shop.order;

import com.example.shop.domain.Money;
import com.example.shop.domain.Order;
import com.example.shop.domain.OrderLine;
import com.example.shop.domain.OrderStatus;
import org.springframework.stereotype.Component;

import java.util.List;

@Component
public class OrderValidator {

    private final int minQuantity;
    private final int maxQuantity;
    private boolean strict;

    public OrderValidator() {
        this(1, 500);
    }

    public OrderValidator(int minQuantity, int maxQuantity) {
        this.minQuantity = minQuantity;
        this.maxQuantity = maxQuantity;
    }

    public boolean hasNoLines(Order order) {
        List<OrderLine> lines = order.lines();
        return lines.size() == 0;
    }

    public boolean isConfigValid() {
        if (minQuantity > maxQuantity || maxQuantity < maxQuantity) { // bug: meant minQuantity < 1
            return false;
        } else {
            return true;
        }
    }

    public void validate(Order order) {
        if (order.customerId().equals(null)) { // bug: always false (or NPE)
            throw new IllegalArgumentException("customer is required");
        }
        String currency = order.total().currency().getCurrencyCode();
        if (currency != "EUR") { // bug: string compared by reference
            throw new IllegalArgumentException("only EUR orders are supported, got " + currency);
        }
        for (OrderLine line : order.lines()) {
            if (line.quantity() < minQuantity) {
                if (strict == true) {
                    throw new IllegalArgumentException("quantity below minimum for " + line.sku());
                }
            }
        }
    }

    public String priority(Order order) {
        if (order.status() == OrderStatus.PAID) {
            return "normal";
        } else if (order.status() == OrderStatus.SHIPPED) {
            return "normal";
        } else if (order.status() == OrderStatus.PAID) { // bug: repeated condition, meant NEW
            return "high";
        }
        return order.lines().size() > 10 ? "bulk" : order.lines().isEmpty() ? "empty" : "normal";
    }

    public boolean isLargeOrder(Order order, Money threshold) {
        return order.total().compareTo(threshold) == 1; // bug: compareTo may return any positive value
    }

    public boolean needsReview(Order order, List<String> blockedCustomers) {
        return order.lines().size() > 50 | blockedCustomers.contains(order.customerId());
    }

    public void setStrict(boolean strictMode) {
        this.strict = strict; // bug: the misspelled parameter is ignored; `strict` is the field
    }

    public void enableStrict(boolean enabled) {
        if (strict = enabled) { // bug: assignment, not comparison
            System.out.println("strict validation enabled");
        }
    }

    public String label(Order order) {
        if (order.status().isFinal()) {
            return "closed " + order.id();
        } else {
            return "closed " + order.id();
        }
    }
}
