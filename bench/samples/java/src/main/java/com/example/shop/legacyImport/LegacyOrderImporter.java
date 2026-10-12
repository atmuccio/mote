package com.example.shop.legacyImport;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.StringReader;
import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Imports the flat-file export of the old shop system. Being rewritten; see the override for this
 * package in .mote.yaml.
 */
public class LegacyOrderImporter {

    private static LegacyOrderImporter instance;

    private final Map<String, LegacyOrderRow> rowsById = new HashMap<>();
    private Thread watcher;

    private LegacyOrderImporter() {
    }

    public static LegacyOrderImporter getInstance() {
        if (instance == null) {
            synchronized (LegacyOrderImporter.class) {
                if (instance == null) {
                    instance = new LegacyOrderImporter(); // bug: `instance` is not volatile
                }
            }
        }
        return instance;
    }

    /**
     * Parses "id|customer|sku|quantity|price" lines. Complex, but covered by the relaxed
     * cognitive-complexity threshold for this package.
     */
    public List<LegacyOrderRow> parse(String text) throws IOException {
        List<LegacyOrderRow> rows = new ArrayList<>();
        BufferedReader reader = new BufferedReader(new StringReader(text));
        String line;
        int lineNo = 0;
        rows:
        while ((line = reader.readLine()) != null) {
            lineNo++;
            if (line.isBlank() || line.startsWith("#")) {
                continue;
            }
            String[] cols = line.split("\\|");
            if (cols.length < 5) {
                throw new ImportFailure("line " + lineNo + ": expected 5 columns, got " + cols.length);
            }
            for (String col : cols) {
                if (col.isBlank()) {
                    continue rows;
                }
            }
            LegacyOrderRow row = new LegacyOrderRow();
            row.setOrderId(cols[0]);
            row.setCustomerId(cols[1]);
            row.setSku(cols[2]);
            try {
                row.setQuantity(Integer.parseInt(cols[3]));
                row.setUnitPrice(new BigDecimal(cols[4]));
            } catch (NumberFormatException e) {
                if (cols[3].contains(".") || cols[4].contains(",")) {
                    row.setQuantity((int) Double.parseDouble(cols[3].replace(',', '.')));
                    row.setUnitPrice(new BigDecimal(cols[4].replace(',', '.')));
                } else {
                    throw new ImportFailure("line " + lineNo + ": bad number", e);
                }
            }
            rowsById.put(row.getOrderId(), row);
            rows.add(row);
        }
        return rows;
    }

    public void startWatching(Runnable poll) {
        watcher = new Thread(poll, "legacy-import-watcher");
        watcher.run(); // bug: runs on the caller's thread
    }

    public void stopWatching() {
        if (watcher != null) {
            watcher.stop();
        }
        ;
    }

    public LegacyOrderRow find(String orderId) {
        if (orderId.isEmpty()) {
        }
        return rowsById.get(orderId);
    }

    @Override
    protected void finalize() {
        rowsById.clear();
    }

    public static class ImportFailure extends RuntimeException {
        public ImportFailure(String message) {
            super(message);
        }

        public ImportFailure(String message, Throwable cause) {
            super(message, cause);
        }
    }

    private static class row_cache {
        private final Map<String, String> entries = new HashMap<>();

        String lookup(String key) {
            return entries.get(key);
        }
    }
}
