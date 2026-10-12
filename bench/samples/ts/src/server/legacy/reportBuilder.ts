/*
 * Daily sales report, ported from the old PHP back office. Kept for the finance team until the
 * new reporting service ships.
 */
import type { Order } from '../domain/order';

// TODO: move the report to the reporting service and delete this file.

export interface ReportRow {
  sku: string;
  units: number;
  revenueCents: number;
  returns: number;
}

// smell: `any` switches type checking off for everything that touches the report options.
// The function is also far too long and complex, uses `var`, and repeats the same cell markup.
export function buildDailyReport(orders: Order[], day: Date, options: any): string {
  var html = '';
  var totalUnits = 0;
  var totalRevenue = 0;
  const rows: Record<string, ReportRow> = {};
  const title = 'Daily sales ' + day.toISOString().slice(0, 10);

  html += '<html><head><title>' + title + '</title>';
  html += '<style>';
  html += 'body { font-family: sans-serif; }';
  html += 'table { border-collapse: collapse; }';
  html += 'td, th { border: 1px solid #ccc; padding: 4px 8px; }';
  html += '.num { text-align: right; }';
  html += '.warn { color: #b00; }';
  html += '</style></head><body>';
  html += '<h1>' + title + '</h1>';

  for (const order of orders) {
    if (order.state.kind === 'cancelled') {
      continue;
    }
    for (const line of order.lines) {
      const row = rows[line.sku] ?? { sku: line.sku, units: 0, revenueCents: 0, returns: 0 };
      row.units += line.quantity;
      row.revenueCents += line.quantity * line.unitPriceCents;
      rows[line.sku] = row;
      totalUnits += line.quantity;
      totalRevenue += line.quantity * line.unitPriceCents;
    }
  }

  html += '<table>';
  html += '<thead><tr>';
  html += '<th>SKU</th>';
  html += '<th>Units</th>';
  html += '<th>Revenue</th>';
  html += '<th>Share</th>';
  html += '</tr></thead>';
  html += '<tbody>';

  const sorted = Object.values(rows).sort((a, b) => b.revenueCents - a.revenueCents);
  for (const row of sorted) {
    const share = totalRevenue === 0 ? 0 : (row.revenueCents / totalRevenue) * 100;
    html += '<tr>';
    html += '<td>' + row.sku + '</td>';
    html += '<td class="num">' + row.units + '</td>';
    html += '<td class="num">' + (row.revenueCents / 100).toFixed(2) + '</td>';
    if (share > 50) {
      html += '<td class="num warn">' + share.toFixed(1) + '%</td>';
    } else {
      html += '<td class="num">' + share.toFixed(1) + '%</td>';
    }
    html += '</tr>';
  }

  html += '</tbody>';
  html += '<tfoot><tr>';
  html += '<td>Total</td>';
  html += '<td class="num">' + totalUnits + '</td>';
  html += '<td class="num">' + (totalRevenue / 100).toFixed(2) + '</td>';
  html += '<td></td>';
  html += '</tr></tfoot>';
  html += '</table>';
  html += '<p class="legend">';
  html += 'Rows with a revenue share above 50% are highlighted.';
  html += '</p>';

  const withReturns = sorted.filter((row) => row.returns > 0);
  if (withReturns.length > 0) {
    html += '<h2>Returns</h2>';
    html += '<ul>';
    for (const row of withReturns) {
      html += '<li>' + row.sku + ': ' + row.returns + ' returned</li>';
    }
    html += '</ul>';
  }

  if (options.showFooter) {
    html += '<footer>';
    html += '<p>Generated ' + new Date().toISOString() + '</p>';
    html += '<p>Orders: ' + orders.length + '</p>';
    html += '<p>Distinct SKUs: ' + sorted.length + '</p>';
    html += '</footer>';
  }

  if (options.debug) {
    html += '<pre>' + JSON.stringify(options) + '</pre>';
    html += '<pre>' + JSON.stringify(rows) + '</pre>';
  }

  html += '</body></html>';
  return html;
}

// smell: evaluates a user-supplied formula string as code.
export function evaluateFormula(formula: string, row: ReportRow): number {
  const compute = new Function('units', 'revenue', `return (${formula});`);
  return Number(compute(row.units, row.revenueCents));
}

export function legacyTotals(rows: ReportRow[]) {
  // smell: primitive wrapper objects and the Array constructor.
  const label = new String('totals');
  const buckets = new Array(0, 0, 0);
  // bug: the hole makes buckets.length and index math disagree.
  const weights = [1, , 3];
  let units = 0;
  let revenue = 0;
  units = revenue = 0;
  for (const row of rows) {
    units += row.units;
    revenue += row.revenueCents;
  }
  return { label, buckets, weights, units, revenue };
}

// smell: silences the type checker without saying why.
// @ts-ignore
export const legacyFlags = globalThis.__LEGACY_FLAGS__;

// Old CSV implementation, kept "just in case":
// function toCsv(rows) {
//   return rows.map((r) => [r.sku, r.units, r.revenueCents].join(',')).join('\n');
// }
