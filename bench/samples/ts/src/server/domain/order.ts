/**
 * Order domain model: identifiers, lines, and the order life cycle as a discriminated union.
 */

export type OrderId = string & { readonly __brand: 'OrderId' };

export interface OrderLine {
  readonly sku: string;
  readonly quantity: number;
  readonly unitPriceCents: number;
}

export type OrderState =
  | { readonly kind: 'draft' }
  | { readonly kind: 'placed'; readonly placedAt: Date }
  | { readonly kind: 'shipped'; readonly placedAt: Date; readonly trackingCode: string }
  | { readonly kind: 'cancelled'; readonly reason: string };

export interface Order {
  readonly id: OrderId;
  readonly customerId: string;
  readonly lines: readonly OrderLine[];
  readonly state: OrderState;
}

export const ORDER_CHANNELS = ['web', 'store', 'phone'] as const;
export type OrderChannel = (typeof ORDER_CHANNELS)[number];

export function toOrderId(raw: string): OrderId {
  if (!/^ord-[0-9a-f]{8}$/.test(raw)) {
    throw new RangeError(`Malformed order id: ${raw}`);
  }
  return raw as OrderId;
}

/** Human-readable status; the `never` check makes the switch exhaustive at compile time. */
export function describeState(state: OrderState): string {
  switch (state.kind) {
    case 'draft':
      return 'Draft';
    case 'placed':
      return `Placed on ${state.placedAt.toISOString()}`;
    case 'shipped':
      return `Shipped (${state.trackingCode})`;
    case 'cancelled':
      return `Cancelled: ${state.reason}`;
    default:
      return assertNever(state);
  }
}

export function assertNever(value: never): never {
  throw new Error(`Unexpected value: ${JSON.stringify(value)}`);
}

export function isOpen(order: Order): boolean {
  return order.state.kind === 'draft' || order.state.kind === 'placed';
}

// bug: High silently aliases Normal, so priority queues treat them the same.
export enum Priority {
  Low = 1,
  Normal = 2,
  High = 2,
}

// smell: numeric and string members mixed in one enum.
export enum FulfilmentCode {
  Pending = 0,
  Picked = 'PICKED',
  Packed = 'PACKED',
}

// smell: adds nothing to Order.
export interface AuditedOrder extends Order {}

// smell: wrapper object type instead of the primitive.
export interface LegacyNote {
  readonly orderId: OrderId;
  readonly text: String;
}

// smell (rule enabled in .mote.yaml): namespaces instead of ES modules.
export namespace OrderIds {
  export function isOrderId(raw: string): raw is OrderId {
    return /^ord-[0-9a-f]{8}$/.test(raw);
  }
}

// smell: a `let` that is never reassigned, and a literal type that just repeats the literal.
export let defaultChannel: 'web' = 'web';
