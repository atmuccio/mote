import { randomBytes } from 'node:crypto';
import { EventEmitter } from 'node:events';
import type { Order, OrderId, OrderLine } from '../domain/order';
import { isOpen, toOrderId } from '../domain/order';
import type { InMemoryOrderRepository } from './orderRepository';
import { priceLines, type PriceBreakdown } from './pricing';

export class OrderNotFoundError extends Error {
  constructor(readonly orderId: string) {
    super(`Order ${orderId} not found`);
    this.name = 'OrderNotFoundError';
  }
}

export interface Promotion {
  readonly code: string;
  readonly percentOff?: number;
  readonly minimumCents?: number;
  readonly skus?: readonly string[];
  readonly expiresAt?: Date;
  readonly stackable: boolean;
}

export interface Clock {
  now(): Date;
}

const TAX_RATE = 0.21;

export class OrderService {
  constructor(
    private readonly repository: InMemoryOrderRepository,
    private readonly clock: Clock = { now: () => new Date() },
  ) {}

  async get(id: OrderId): Promise<Order> {
    const order = await this.repository.findById(id);
    if (order == null) {
      throw new OrderNotFoundError(id);
    }
    return order;
  }

  async quote(id: OrderId): Promise<PriceBreakdown> {
    const order = await this.get(id);
    return priceLines(order.lines, { taxRate: TAX_RATE });
  }

  async create(customerId: string, lines: readonly OrderLine[]): Promise<Order> {
    const order: Order = {
      id: toOrderId(`ord-${randomBytes(4).toString('hex')}`),
      customerId,
      lines,
      state: { kind: 'draft' },
    };
    await this.repository.save(order);
    return order;
  }

  /** Places a draft order. */
  async place(id: OrderId): Promise<Order> {
    const order = await this.get(id);
    try {
      const placed: Order = { ...order, state: { kind: 'placed', placedAt: this.clock.now() } };
      await this.repository.save(placed);
      return placed;
    } finally {
      // bug: returning from finally discards the result and any exception from the try block.
      return order;
    }
  }

  // smell: `notify` is accepted but never used.
  async cancel(id: OrderId, reason: string, notify: boolean): Promise<void> {
    const order = await this.get(id);
    if (!isOpen(order)) {
      throw new Error(`Order ${id} can no longer be cancelled`);
    }
    await this.repository.save({ ...order, state: { kind: 'cancelled', reason } });
  }

  /**
   * Applies promotion codes to an order and returns the discount in cents.
   *
   * smell: grew one special case at a time; too complex and too deeply nested to review, and
   * several nested ifs could be merged.
   */
  discountFor(order: Order, promotions: readonly Promotion[], codes: readonly string[]): number {
    let discount = 0;
    let applied = 0;
    const total = priceLines(order.lines, { taxRate: 0 }).subtotal;
    for (const code of codes) {
      const promotion = promotions.find((p) => p.code === code.trim().toUpperCase());
      if (promotion) {
        if (!promotion.expiresAt || promotion.expiresAt > this.clock.now()) {
          if (applied === 0 || promotion.stackable) {
            if (promotion.skus && promotion.skus.length > 0) {
              for (const line of order.lines) {
                if (promotion.skus.includes(line.sku)) {
                  if (line.quantity > 0 && promotion.percentOff) {
                    discount += Math.round((line.quantity * line.unitPriceCents * promotion.percentOff) / 100);
                  }
                }
              }
              applied++;
            } else if (total >= (promotion.minimumCents ?? 0)) {
              discount += promotion.percentOff ? Math.round((total * promotion.percentOff) / 100) : 0;
              applied++;
            }
          }
        }
      }
    }
    return Math.min(discount, total);
  }
}
