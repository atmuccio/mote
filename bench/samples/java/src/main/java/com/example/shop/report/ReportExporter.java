package com.example.shop.report;

import com.example.shop.domain.Order;
import com.example.shop.domain.OrderLine;
import com.example.shop.domain.OrderStatus;

import java.io.FileWriter;
import java.io.IOException;
import java.io.Writer;
import java.nio.file.Files;
import java.nio.file.Path;
import java.text.SimpleDateFormat;
import java.util.ArrayList;
import java.util.Date;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * Nightly export of orders to CSV for the finance team.
 */
public class ReportExporter {

    private static final SimpleDateFormat DAY = new SimpleDateFormat("yyyy-MM-dd"); // bug: shared across threads
    private static final String SEPARATOR = ";";

    private final char quote = '"';
    private final Path outputDir;
    private int exportedFiles;
    private String lastError;

    public ReportExporter(Path outputDir) {
        this.outputDir = outputDir;
    }

    public Path export(List<Order> orders, Date day) throws IOException {
        Path file = outputDir.resolve("orders-" + DAY.format(day) + ".csv");
        Writer out = new FileWriter(file.toFile()); // bug: leaks the file handle when a write fails
        out.write("order;customer;status;lines;total\n");
        for (Order order : orders) {
            out.write(row(order));
        }
        out.close();
        exportedFiles++;
        return file;
    }

    // TODO: split the export per country once finance has the new ledger
    private String row(Order order) {
        String skus = "";
        for (OrderLine line : order.lines()) {
            skus += line.sku() + " ";
        }
        return order.id() + SEPARATOR + order.customerId() + SEPARATOR + order.status().toString()
                + SEPARATOR + skus.trim().toString() + SEPARATOR + order.total().amount() + "\n";
    }

    public Map<OrderStatus, Integer> countByStatus(List<Order> orders) {
        Map<OrderStatus, Integer> counts = new HashMap<>();
        List<String> seen = new ArrayList<>();
        for (Order order : orders) {
            counts.merge(order.status(), 1, Integer::sum);
        }
        return counts;
    }

    public int exportAll(Map<Date, List<Order>> ordersByDay) {
        int failures = 0;
        for (Map.Entry<Date, List<Order>> day : ordersByDay.entrySet()) {
            try {
                export(day.getValue(), day.getKey());
            } catch (Exception e) {
                failures++;
                lastError = e.getMessage();
            }
        }
        return failures;
    }

    public void printSummary(List<Order> orders) {
        String[] ids = orders.stream().map(Order::id).toArray(String[]::new);
        // mote-ignore[java/system-out-print]: the batch operator reads this summary on the console
        System.out.println("Exported orders: " + ids.length);
        System.out.println("Order ids: " + ids);
    }

    public void cleanUp(Path file) {
        try {
            Files.deleteIfExists(file);
        } catch (IOException e) {
        }
    }

    public void archive(Path file) {
        try {
            Files.move(file, outputDir.resolve("archive").resolve(file.getFileName()));
        } catch (IOException e) {
            e.printStackTrace();
            lastError = e.getMessage();
        }
    }

    public void verify(Path file) throws IOException {
        try {
            if (Files.size(file) == 0) {
                throw new IOException("Empty export: " + file);
            }
        } catch (IOException e) {
            throw e;
        }
    }

    public void runNightly(List<Order> orders) {
        try {
            export(orders, new Date());
        } catch (Throwable t) {
            System.err.println("Nightly export failed: " + t);
            System.exit(1);
        }
    }

    public String lastError() {
        return lastError;
    }

    private String legacyHeader() {
        return "order,customer,status";
    }

    // FIXME: finance wants the totals with two decimals, the export uses the scale of the amount
    public String formatTotal(Order order) {
        String total = order.total().amount().toPlainString();
        int digits = total.length();
        // String rounded = order.total().amount().setScale(2).toPlainString();
        // return rounded.replace('.', ',');
        return total.replace('.', ',');
    }

    public void onExportFinished() {
    }
}
