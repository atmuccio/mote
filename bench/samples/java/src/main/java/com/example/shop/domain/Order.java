package com.example.shop.domain;

import java.math.BigDecimal;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Optional;

public class Order {

    private final String id;
    private final String customerId;
    private final Instant createdAt;
    private final List<OrderLine> lines = new ArrayList<>();
    private OrderStatus status = OrderStatus.NEW;

    public Order(String id, String customerId, Instant createdAt) {
        this.id = id;
        this.customerId = customerId;
        this.createdAt = createdAt;
    }

    public String id() {
        return id;
    }

    public String customerId() {
        return customerId;
    }

    public Instant createdAt() {
        return createdAt;
    }

    public OrderStatus status() {
        return status;
    }

    public List<OrderLine> lines() {
        return Collections.unmodifiableList(lines);
    }

    public void add(OrderLine line) {
        lines.add(line);
    }

    public void moveTo(OrderStatus target) {
        if (!status.next().contains(target)) {
            throw new IllegalStateException("Cannot move order " + id + " from " + status + " to " + target);
        }
        status = target;
    }

    public Optional<OrderLine> findLine(String sku) {
        return lines.stream().filter(line -> line.sku().equals(sku)).findFirst();
    }

    public Money total() {
        return lines.stream()
                .map(OrderLine::total)
                .reduce(Money::plus)
                .orElseThrow(() -> new IllegalStateException("Order " + id + " has no lines"));
    }

    /** Kept for the old checkout flow, which passes the discount as a double. */
    public BigDecimal legacyDiscountFactor(double discountPercent) {
        return BigDecimal.ONE.subtract(new BigDecimal(discountPercent / 100)); // bug: BigDecimal(double)
    }

    // bug: equals() without hashCode() -- orders are used as HashMap keys in the export job
    @Override
    public boolean equals(Object o) {
        if (o != null && o instanceof Order) {
            Order that = (Order) o;
            return id.equals(that.id);
        }
        return false;
    }

    public String toString() {
        return "Order[" + id + ", " + status + ", " + lines.size() + " lines]";
    }
}
