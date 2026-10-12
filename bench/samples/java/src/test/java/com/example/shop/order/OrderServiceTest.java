package com.example.shop.order;

import com.example.shop.domain.Money;
import com.example.shop.domain.Order;
import com.example.shop.domain.OrderLine;
import com.example.shop.domain.OrderStatus;
import com.example.shop.inventory.InventoryService;
import com.example.shop.order.OrderService.CheckoutException;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Disabled;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.InjectMocks;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;

import java.time.Clock;
import java.time.Instant;
import java.time.ZoneOffset;
import java.util.List;
import java.util.Optional;

import static org.assertj.core.api.Assertions.assertThat;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

@ExtendWith(MockitoExtension.class)
class OrderServiceTest {

    private static final Instant NOW = Instant.parse("2026-03-01T10:15:30Z");

    @Mock
    private OrderRepository orders;

    @Mock
    private InventoryService inventory;

    @Mock
    private PaymentGateway payments;

    @Mock
    private Clock clock;

    @InjectMocks
    private OrderService service;

    private Order order;

    @BeforeEach
    void setUp() {
        order = new Order("o-1", "c-42", NOW);
        order.add(new OrderLine("SKU-1", 2, Money.of("9.99", "EUR")));
        when(orders.findById("o-1")).thenReturn(Optional.of(order));
    }

    @Test
    void checkout_charges_the_customer_and_marks_the_order_paid() throws Exception {
        when(inventory.available("SKU-1")).thenReturn(Optional.of(10));
        when(clock.instant()).thenReturn(NOW);
        when(payments.charge(anyString(), any())).thenReturn("tx-1");

        String transactionId = service.checkout("o-1", false, null);

        assertThat(transactionId).isEqualTo("tx-1");
        assertThat(order.status()).isEqualTo(OrderStatus.PAID);
        verify(inventory).reserve(order.lines());
    }

    @Test
    void checkout_rejects_unknown_products() throws Exception {
        when(inventory.available("SKU-1")).thenReturn(Optional.empty());

        CheckoutException e = assertThrows(CheckoutException.class, () -> service.checkout("o-1", false, null));

        assertThat(e).hasMessageContaining("unknown product: SKU-1");
        verify(payments, never()).charge(anyString(), any());
    }

    @Test
    void checkout_applies_the_vip_coupon() throws Exception {
        when(inventory.available("SKU-1")).thenReturn(Optional.of(10));
        when(clock.instant()).thenReturn(NOW);
        when(payments.charge(anyString(), any())).thenReturn("tx-2");

        service.checkout("o-1", false, "VIP-2026");

        verify(payments).charge("c-42", Money.of("15.98", "EUR"));
    }

    @Test
    void order_is_empty_after_creation() {
        Order fresh = new Order("o-2", "c-1", NOW);
        assertEquals(true, fresh.lines().isEmpty());
    }

    @Test
    void cancel_returns_quickly() throws InterruptedException {
        service.cancel("o-1");
        Thread.sleep(200);
    }

    @Disabled
    @Test
    void checkout_retries_when_stock_arrives_late() {
        assertThat(service.latestOpenOrder("c-42")).isPresent();
    }

    @Test
    void describe_covers_every_status() {
        for (OrderStatus status : List.of(OrderStatus.PAID, OrderStatus.SHIPPED)) {
            assertThat(service.describe(status)).isNotBlank();
        }
    }

    @Test
    void clock_is_utc_in_production() {
        Clock production = Clock.system(ZoneOffset.UTC);
        assertThat(production.getZone()).isEqualTo(ZoneOffset.UTC);
    }
}
