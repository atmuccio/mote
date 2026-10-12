import type { Order, OrderId, OrderState } from '../domain/order';

export interface Repository<TId, TEntity> {
  findById(id: TId): Promise<TEntity | undefined>;
  save(entity: TEntity): Promise<void>;
  delete(id: TId): Promise<boolean>;
}

export interface OrderQuery {
  readonly customerId?: string;
  readonly states?: ReadonlyArray<OrderState['kind']>;
  readonly limit?: number;
}

/** In-memory repository used by the service in development and by the tests. */
export class InMemoryOrderRepository implements Repository<OrderId, Order> {
  readonly #orders = new Map<OrderId, Order>();

  async findById(id: OrderId): Promise<Order | undefined> {
    return this.#orders.get(id);
  }

  async save(order: Order): Promise<void> {
    this.#orders.set(order.id, order);
  }

  async delete(id: OrderId): Promise<boolean> {
    return this.#orders.delete(id);
  }

  async query({ customerId, states, limit = 50 }: OrderQuery = {}): Promise<Order[]> {
    const matches = [...this.#orders.values()].filter(
      (order) =>
        (customerId === undefined || order.customerId === customerId) &&
        (states?.includes(order.state.kind) ?? true),
    );
    return matches.slice(0, limit);
  }

  get size(): number {
    return this.#orders.size;
  }
}

/** Retries a repository call with exponential back-off; the last error is rethrown. */
export async function withRetry<T>(
  operation: () => Promise<T>,
  { attempts = 3, baseDelayMs = 50 }: { attempts?: number; baseDelayMs?: number } = {},
): Promise<T> {
  let lastError: unknown;
  for (let attempt = 0; attempt < attempts; attempt++) {
    try {
      return await operation();
    } catch (error) {
      lastError = error;
      await new Promise<void>((resolve) => setTimeout(resolve, baseDelayMs * 2 ** attempt));
    }
  }
  throw lastError;
}
