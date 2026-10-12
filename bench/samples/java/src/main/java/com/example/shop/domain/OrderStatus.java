package com.example.shop.domain;

import java.util.EnumSet;
import java.util.Set;

public enum OrderStatus {
    NEW,
    PAID,
    SHIPPED,
    DELIVERED,
    CANCELLED;

    /** Statuses an order may move to from this one. */
    public Set<OrderStatus> next() {
        return switch (this) {
            case NEW -> EnumSet.of(PAID, CANCELLED);
            case PAID -> EnumSet.of(SHIPPED, CANCELLED);
            case SHIPPED -> EnumSet.of(DELIVERED);
            case DELIVERED, CANCELLED -> EnumSet.noneOf(OrderStatus.class);
        };
    }

    public boolean isFinal() {
        return next().isEmpty();
    }
}
