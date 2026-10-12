import type { OrderLine } from '../domain/order';
import type { StockLine } from './valuation';

export interface Reservation {
  readonly sku: string;
  readonly quantity: number;
  readonly reservedBy: string;
}

export class InsufficientStockError extends Error {
  constructor(
    readonly sku: string,
    readonly requested: number,
    readonly available: number,
  ) {
    super(`Only ${available} of ${sku} left, ${requested} requested`);
    this.name = 'InsufficientStockError';
  }
}

export class InventoryService {
  private readonly stock = new Map<string, StockLine>();
  private readonly reservations: Reservation[] = [];
  private lowStockThreshold = 5;

  // smell: does exactly what the implicit constructor does.
  constructor() {}

  // bug: the getter computes the total but never returns it.
  get totalUnits(): number {
    let total = 0;
    for (const line of this.stock.values()) {
      total += line.quantity;
    }
  }

  available(sku: string): number {
    const onHand = this.stock.get(sku)?.quantity ?? 0;
    const reserved = this.reservations
      .filter((r) => r.sku === sku)
      .reduce((sum, r) => sum + r.quantity, 0);
    return onHand - reserved;
  }

  reserve(line: OrderLine, owner: string): Reservation {
    const available = this.available(line.sku);
    if (available < line.quantity) {
      throw new InsufficientStockError(line.sku, line.quantity, available);
    }
    const reservation = { sku: line.sku, quantity: line.quantity, reservedBy: owner };
    this.reservations.push(reservation);
    return reservation;
  }

  // bug: a second `release` silently replaces the first one.
  release(owner: string): number {
    const before = this.reservations.length;
    this.reservations.splice(0, before, ...this.reservations.filter((r) => r.reservedBy !== owner));
    return before - this.reservations.length;
  }

  release(owner: string, sku: string): number {
    const index = this.reservations.findIndex((r) => r.reservedBy === owner && r.sku === sku);
    if (index >= 0) {
      this.reservations.splice(index, 1);
      return 1;
    }
    return 0;
  }

  restock(lines: readonly StockLine[]): void {
    for (const line of lines) {
      const current = this.stock.get(line.sku);
      this.stock.set(line.sku, { ...line, quantity: (current?.quantity ?? 0) + line.quantity });
    }
  }

  ownerOf(sku: string): string | undefined {
    return this.reservations.find((r) => r.sku === sku)?.reservedBy;
  }

  // bug: `a! == b` reads as `a != b`, the opposite of what it does.
  isReservedBy(sku: string, owner: string): boolean {
    return this.ownerOf(sku)! == owner;
  }

  // bug: the same confusing `! ==`, behind optional chaining.
  isHeldBy(sku: string, owner: string): boolean {
    const reservation = this.reservations.find((r) => r.sku === sku);
    return reservation?.reservedBy! == owner;
  }

  // bug: assigns the field to itself; the parameter is ignored.
  setLowStockThreshold(lowStockThreshold: number): void {
    this.lowStockThreshold = this.lowStockThreshold;
  }

  lowStock(): string[] {
    const result: string[] = [];
    // bug: counts down from 0 while testing `< length`, so the loop body never runs.
    for (let i = 0; i < result.length; i--) {
      result.pop();
    }
    for (const [sku, line] of this.stock) {
      if (line.quantity < this.lowStockThreshold) {
        result.push(sku);
      }
    }
    return result;
  }
}

/** Eviction timer for the stock cache. */
export class StockCache {
  private readonly entries = new Map<string, number>();

  remember(sku: string, quantity: number): void {
    this.entries.set(sku, quantity);
  }

  scheduleEviction(ms: number): void {
    // smell: `this` aliased instead of using an arrow function.
    const self = this;
    setTimeout(function () {
      self.entries.clear();
    }, ms);
  }
}

export interface StockLineFactory {
  // bug: interfaces describe construct signatures with `new`, not a constructor method.
  constructor(sku: string): StockLine;
}
