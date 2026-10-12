package com.example.shop.inventory;

import java.util.List;
import java.util.Optional;

/**
 * Stock levels per SKU. Implemented by Spring Data at runtime.
 */
public interface StockRepository {

    Optional<StockLevel> findBySku(String sku);

    List<StockLevel> findAll();

    StockLevel save(StockLevel level);

    record StockLevel(String sku, int onHand, int reserved) {

        public int available() {
            return onHand - reserved;
        }

        public StockLevel reserve(int quantity) {
            return new StockLevel(sku, onHand, reserved + quantity);
        }
    }
}
