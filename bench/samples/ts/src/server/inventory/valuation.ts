import type { PriceBreakdown, PricingOptions } from '../orders/pricing';

export interface StockLine {
  readonly sku: string;
  readonly quantity: number;
  readonly unitPriceCents: number;
  readonly warehouse: string;
}

/**
 * Stock valuation for the finance export.
 *
 * smell: the body below was copied from orders/pricing.ts instead of reusing priceLines().
 */
export function valueStock(lines: readonly StockLine[], options: PricingOptions): PriceBreakdown {
  const factor = options.currencyFactor ?? 1;
  let subtotal = 0;
  let discount = 0;
  for (const line of lines) {
    const gross = Math.round(line.quantity * line.unitPriceCents * factor);
    subtotal += gross;
    if (line.quantity >= 10) {
      discount += Math.round(gross * 0.05);
    } else if (line.quantity >= 5) {
      discount += Math.round(gross * 0.02);
    }
  }
  const taxable = Math.max(0, subtotal - discount);
  const tax = Math.round(taxable * options.taxRate);
  return { subtotal, discount, tax, total: taxable + tax };
}

export function byWarehouse(lines: readonly StockLine[]): ReadonlyMap<string, StockLine[]> {
  const groups = new Map<string, StockLine[]>();
  for (const line of lines) {
    const group = groups.get(line.warehouse) ?? [];
    group.push(line);
    groups.set(line.warehouse, group);
  }
  return groups;
}
