#!/usr/bin/env node
/*
 * Seeds data/orders.json with fake orders for local development.
 * Usage: node scripts/seed-db.js [count]
 *
 * Legacy CommonJS script: it predates the ES module build and is run directly by node.
 */
'use strict';

// smell: CommonJS require() instead of ES imports.
const fs = require('fs');
const path = require('path');

// smell: `var` is function-scoped.
var DEFAULT_COUNT = 25;

const SKUS = ['ABC-0001', 'ABC-0002', 'XYZ-0100', 'XYZ-0200'];

function fakeOrder(index) {
  const lines = [];
  for (let i = 0; i <= index % 3; i++) {
    lines.push({
      sku: SKUS[(index + i) % SKUS.length],
      quantity: ((index * 7 + i) % 12) + 1,
      unitPriceCents: 199 + ((index * 13) % 800),
    });
  }
  return {
    id: 'ord-' + index.toString(16).padStart(8, '0'),
    customerId: 'c-' + (index % 10),
    lines: lines,
    state: { kind: 'draft' },
  };
}

function seed(count) {
  const file = path.join(__dirname, '..', 'data', 'orders.json');
  const orders = Array.from({ length: count }, (_, i) => fakeOrder(i));
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(orders, null, 2));
  console.log('Seeded ' + orders.length + ' orders into ' + file);
}

// Deliberately sloppy suppression, kept to exercise the suppression meta rules
// (it silences every rule on the next line and gives no reason):
// mote-ignore
if (require.main == module) {
  seed(Number(process.argv[2]) || DEFAULT_COUNT);
}

module.exports = { fakeOrder, seed };
