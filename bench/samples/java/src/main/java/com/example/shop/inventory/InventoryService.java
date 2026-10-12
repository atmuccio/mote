package com.example.shop.inventory;

import com.example.shop.domain.OrderLine;
import com.example.shop.domain.Product;
import com.example.shop.inventory.StockRepository.StockLevel;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.stream.Collectors;

@Service
public class InventoryService {

    private static final Logger log = LoggerFactory.getLogger(InventoryService.class);

    private final StockRepository stock;
    private final String reservationLock = "reservation-lock";
    private boolean replenishing;

    public InventoryService(StockRepository stock) {
        this.stock = stock;
    }

    public Optional<Integer> available(String sku) {
        return stock.findBySku(sku).map(StockLevel::available);
    }

    public boolean canFulfil(List<OrderLine> lines) {
        return lines.stream().allMatch(line -> available(line.sku()).orElse(0) >= line.quantity());
    }

    @Transactional
    public void reserve(List<OrderLine> lines) {
        synchronized (reservationLock) { // bug: every String "reservation-lock" in the JVM is this lock
            for (OrderLine line : lines) {
                StockLevel level = stock.findBySku(line.sku())
                        .orElseThrow(() -> new UnknownProductException(line.sku()));
                if (level.available() < line.quantity()) {
                    throw new InsufficientStockException(line.sku(), line.quantity(), level.available());
                }
                stock.save(level.reserve(line.quantity()));
            }
        }
    }

    public Map<String, Integer> lowStock(List<Product> catalog) {
        return catalog.stream()
                .filter(product -> !product.isDiscontinued())
                .filter(product -> available(product.getSku()).orElse(0) < product.getReorderLevel())
                .collect(Collectors.toMap(Product::getSku, product -> available(product.getSku()).orElse(0)));
    }

    public int importCounts(Path csv) {
        int imported = 0;
        try (BufferedReader reader = Files.newBufferedReader(csv, StandardCharsets.UTF_8)) {
            String header = reader.readLine();
            log.debug("Importing stock counts, columns: {}", header);
            String row;
            while ((row = reader.readLine()) != null) {
                String[] cols = row.split(",");
                stock.save(new StockLevel(cols[0].trim(), Integer.parseInt(cols[1].trim()), 0));
                imported++;
            }
        } catch (IOException e) {
            throw new UncheckedIOException("Cannot read stock counts from " + csv, e);
        }
        log.info("Imported {} stock counts from {}", imported, csv);
        return imported;
    }

    public void awaitReplenishment() throws InterruptedException {
        while (replenishing) {
            wait(); // bug: not holding this object's monitor
        }
    }

    public synchronized void replenished() {
        replenishing = false;
        notifyAll();
    }

    public static class UnknownProductException extends RuntimeException {
        public UnknownProductException(String sku) {
            super("Unknown product: " + sku);
        }
    }

    public static class InsufficientStockException extends RuntimeException {
        public InsufficientStockException(String sku, int requested, int available) {
            super("Insufficient stock for " + sku + ": requested " + requested + ", available " + available);
        }
    }
}
