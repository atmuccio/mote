package com.example.shop.order;

import com.example.shop.domain.Order;
import com.example.shop.domain.OrderStatus;

import java.util.List;
import java.util.Optional;

public interface OrderRepository {

    Optional<Order> findById(String id);

    List<Order> findByStatus(OrderStatus status);

    Order save(Order order);
}
