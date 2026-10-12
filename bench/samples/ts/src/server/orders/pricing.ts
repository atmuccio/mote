import type { OrderLine } from '../domain/order';

export interface PriceBreakdown {
  readonly subtotal: number;
  readonly discount: number;
  readonly tax: number;
  readonly total: number;
}

export interface PricingOptions {
  readonly taxRate: number;
  readonly currencyFactor?: number;
}

const EMPTY_BREAKDOWN: PriceBreakdown = { subtotal: 0, discount: 0, tax: 0, total: 0 };

/** Volume pricing: 5% off lines of ten or more units, 2% off lines of five or more. */
export function priceLines(lines: readonly OrderLine[], options: PricingOptions): PriceBreakdown {
  if (lines.length === 0) {
    return EMPTY_BREAKDOWN;
  }
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

export function formatCents(cents: number, currency = 'EUR'): string {
  return new Intl.NumberFormat('en-IE', { style: 'currency', currency }).format(cents / 100);
}
