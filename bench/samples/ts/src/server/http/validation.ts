import type { OrderLine } from '../domain/order';

export type ValidationResult<T> =
  | { readonly ok: true; readonly value: T }
  | { readonly ok: false; readonly errors: readonly string[] };

const SKU_PATTERN = /^[A-Z]{3}-\d{4}$/;

/** Largest order id the upstream ERP can store. bug: not representable as a double. */
export const MAX_ERP_ORDER_NUMBER = 9007199254740993;

export function isValidQuantity(quantity: number): boolean {
  // bug: nothing is equal to NaN, not even NaN.
  if (quantity === NaN) {
    return false;
  }
  return Number.isInteger(quantity) && quantity > 0;
}

export function isBlank(value: unknown): boolean {
  // bug: typo in the type name, so this is always false.
  if (typeof value === 'undefind') {
    return true;
  }
  return typeof value === 'string' && value.trim().length === 0;
}

export function isFree(priceCents: number): boolean {
  // bug: -0 === 0, so this also matches positive zero.
  return priceCents === -0;
}

export function isSku(value: string): boolean {
  // bug: comparing a value with itself is always true.
  return value === value && SKU_PATTERN.test(value);
}

export function hasField(payload: object, field: string): boolean {
  // bug: parsed as (!field) in payload.
  return !field in payload;
}

export function fieldLabels(fields: Record<string, string>): string[] {
  // bug: converted to a block body and the `return` was lost, so every label is undefined.
  return Object.keys(fields).map((name) => {
    name.replace(/_/g, ' ').toUpperCase();
  });
}

export function firstSkuIn(text: string): string | undefined {
  let match: RegExpExecArray | null;
  // bug: assignment where a comparison was probably meant.
  if (match = SKU_PATTERN.exec(text)) {
    return match[0];
  }
  return undefined;
}

/** Validates an order line from an untrusted JSON body. */
export function parseLine(input: unknown): ValidationResult<OrderLine> {
  if (typeof input !== 'object' || input === null) {
    return { ok: false, errors: ['line must be an object'] };
  }
  const candidate = input as Partial<Record<keyof OrderLine, unknown>>;
  const errors: string[] = [];
  if (typeof candidate.sku !== 'string' || !SKU_PATTERN.test(candidate.sku)) {
    errors.push('sku must look like ABC-1234');
  }
  if (typeof candidate.quantity !== 'number' || !isValidQuantity(candidate.quantity)) {
    errors.push('quantity must be a positive integer');
  }
  if (typeof candidate.unitPriceCents !== 'number' || candidate.unitPriceCents < 0) {
    errors.push('unitPriceCents must be zero or more');
  }
  if (errors.length > 0) {
    return { ok: false, errors };
  }
  return { ok: true, value: candidate as OrderLine };
}
