import { beforeEach, describe, expect, it } from 'vitest';
import type { Order, OrderId } from '../domain/order';
import { InMemoryOrderRepository } from './orderRepository';
import { OrderNotFoundError, OrderService, type Promotion } from './orderService';

const fixedClock = { now: () => new Date('2026-03-01T10:00:00Z') };

function draftOrder(overrides: Partial<Order> = {}): Order {
  return {
    id: 'ord-0000beef' as OrderId,
    customerId: 'c-42',
    lines: [
      { sku: 'ABC-0001', quantity: 10, unitPriceCents: 250 },
      { sku: 'XYZ-0002', quantity: 1, unitPriceCents: 1999 },
    ],
    state: { kind: 'draft' },
    ...overrides,
  };
}

describe('OrderService', () => {
  let repository: InMemoryOrderRepository;
  let service: OrderService;

  beforeEach(() => {
    repository = new InMemoryOrderRepository();
    service = new OrderService(repository, fixedClock);
  });

  it('rejects unknown orders', async () => {
    await expect(service.get('ord-00000000' as OrderId)).rejects.toBeInstanceOf(OrderNotFoundError);
  });

  it('quotes volume discounts and tax', async () => {
    await repository.save(draftOrder());
    const quote = await service.quote('ord-0000beef' as OrderId);
    // Console output is fine in tests: js/no-console does not apply to test files.
    console.log(quote);
    expect(quote).toEqual({ subtotal: 4499, discount: 125, tax: 918, total: 5292 });
  });

  it('applies a stackable promotion once per code', () => {
    const promotions: Promotion[] = [{ code: 'SPRING10', percentOff: 10, stackable: true }];
    const discount = service.discountFor(draftOrder(), promotions, ['spring10']);
    expect(discount).toBe(450);
  });

  it('ignores expired promotions', () => {
    const expired: Promotion = { code: 'OLD', percentOff: 50, stackable: false, expiresAt: new Date('2025-01-01') };
    expect(service.discountFor(draftOrder(), [expired], ['OLD'])).toBe(0);
  });
});
