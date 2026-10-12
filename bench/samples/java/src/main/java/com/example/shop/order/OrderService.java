package com.example.shop.order;

import com.example.shop.domain.Money;
import com.example.shop.domain.Order;
import com.example.shop.domain.OrderLine;
import com.example.shop.domain.OrderStatus;
import com.example.shop.inventory.InventoryService;
import com.example.shop.order.PaymentGateway.PaymentException;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;

import java.math.BigDecimal;
import java.time.Clock;
import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;

@Service
public class OrderService {

    private static final Logger log = LoggerFactory.getLogger(OrderService.class);
    private static final Duration PAYMENT_TIMEOUT = Duration.ofSeconds(30);

    @Autowired
    private OrderRepository orders;

    @Autowired
    private InventoryService inventory;

    @Autowired
    private PaymentGateway payments;

    @Autowired
    private Clock clock;

    private int retryLimit = 3;

    public Order get(String id) {
        return orders.findById(id).orElseThrow(() -> new OrderNotFoundException(id));
    }

    public Optional<Order> latestOpenOrder(String customerId) {
        List<Order> open = orders.findByStatus(OrderStatus.NEW);
        if (open.isEmpty()) {
            return null; // bug: callers write latestOpenOrder(id).isPresent()
        }
        return open.stream()
                .filter(order -> order.customerId().equals(customerId))
                .reduce((first, second) -> second);
    }

    public Order reload(String id) {
        Optional<Order> found = orders.findById(id);
        log.debug("Reloading order {}", id);
        return found.get(); // bug: no presence check
    }

    public void setRetryLimit(int retryLimit) {
        retryLimit = retryLimit; // bug: assigns the parameter to itself; the field keeps its value
    }

    /**
     * Checks out an order: validates it, reserves stock, applies the coupon, charges the customer
     * and schedules shipping. Grew one special case at a time.
     */
    public String checkout(String orderId, boolean expedite, String couponCode) {
        Order order = get(orderId);
        List<String> problems = new ArrayList<>();
        String transactionId = null;
        if (order.status() == OrderStatus.NEW) {
            if (order.customerId() == "guest") { // bug: string compared by reference
                problems.add("guest checkout is disabled");
            }
            for (OrderLine line : order.lines()) {
                if (line.quantity() > 0) {
                    if (inventory.available(line.sku()).isPresent()) {
                        for (int attempt = 0; attempt < retryLimit; attempt++) {
                            if (inventory.available(line.sku()).orElse(0) >= line.quantity()) {
                                if (expedite) {
                                    log.info("Expedited line {} of order {}", line.sku(), orderId);
                                }
                                break;
                            } else if (attempt == retryLimit - 1) {
                                problems.add("out of stock: " + line.sku());
                            }
                        }
                    } else {
                        problems.add("unknown product: " + line.sku());
                    }
                }
            }
            Money total = order.total();
            if (couponCode != null && !couponCode.isBlank()) {
                if (couponCode.startsWith("VIP")) {
                    total = total.plus(total.percent(new BigDecimal("-20")));
                } else if (couponCode.startsWith("WELCOME")) {
                    if (orders.findByStatus(OrderStatus.DELIVERED).stream()
                            .noneMatch(previous -> previous.customerId().equals(order.customerId()))) {
                        total = total.plus(total.percent(new BigDecimal("-10")));
                    } else {
                        problems.add("welcome coupon already used");
                    }
                } else {
                    problems.add("unknown coupon: " + couponCode);
                }
            }
            if (problems.isEmpty()) {
                Instant deadline = clock.instant().plus(PAYMENT_TIMEOUT);
                try {
                    transactionId = payments.charge(order.customerId(), total);
                    inventory.reserve(order.lines());
                    order.moveTo(OrderStatus.PAID);
                    orders.save(order);
                } catch (PaymentException e) {
                    throw new CheckoutException("Payment failed for order " + orderId); // bug: cause dropped
                }
                if (clock.instant().isAfter(deadline)) {
                    log.warn("Payment for order {} took longer than {}", orderId, PAYMENT_TIMEOUT);
                }
                if (expedite) {
                    if (total.compareTo(Money.of("100.00", "EUR")) >= 0) {
                        log.info("Free express shipping for order {}", orderId);
                    } else {
                        log.info("Paid express shipping for order {}", orderId);
                    }
                }
            } else {
                log.warn("Order {} cannot be checked out: {}", orderId, problems);
            }
        } else if (order.status() == OrderStatus.PAID) {
            log.info("Order {} is already paid", orderId);
        } else {
            throw new CheckoutException("Order " + orderId + " is " + order.status());
        }
        if (!problems.isEmpty()) {
            throw new CheckoutException(String.join("; ", problems));
        }
        return transactionId;
    }

    public String describe(OrderStatus status) {
        String text = "";
        switch (status) {
            case NEW:
                text = "waiting for payment";
            case PAID: // bug: NEW falls through and is described as "paid"
                text = "paid";
                break;
            case SHIPPED:
                text = "on its way";
                break;
        }
        return text;
    }

    public boolean cancel(String orderId) {
        Order order = get(orderId);
        try {
            order.moveTo(OrderStatus.CANCELLED);
            orders.save(order);
        } finally {
            return true; // bug: swallows the IllegalStateException thrown by moveTo
        }
    }

    public void refund(String transactionId, Money amount) throws Exception {
        if (transactionId == null) {
            throw new IllegalArgumentException("transactionId");
            log.warn("Refund without a transaction"); // bug: unreachable
        }
        payments.refund(transactionId, amount);
    }

    public Order createOrder(String customerId, String street, String city, String postCode,
                             String country, String email, String phone, boolean giftWrap) {
        Order order = new Order(java.util.UUID.randomUUID().toString(), customerId, clock.instant());
        log.debug("New order for {} ({}, {} {} {}, {}, {}, gift wrap: {})",
                customerId, street, postCode, city, country, email, phone, giftWrap);
        return orders.save(order);
    }

    public static class OrderNotFoundException extends RuntimeException {
        public OrderNotFoundException(String id) {
            super("No order " + id);
        }
    }

    public static class CheckoutException extends RuntimeException {
        public CheckoutException(String message) {
            super(message);
        }
    }
}
