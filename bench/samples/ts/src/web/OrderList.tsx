import { memo, useCallback, useMemo, useState, type ChangeEvent } from 'react';
import { assertNever, describeState, type Order } from '../server/domain/order';
import { formatCents, priceLines } from '../server/orders/pricing';
import { useOrders } from './useOrders';

interface OrderListProps {
  readonly customerId: string;
  readonly onSelect?: (order: Order) => void;
}

const OrderRow = memo(function OrderRow({ order, onSelect }: { order: Order; onSelect: (order: Order) => void }) {
  const total = priceLines(order.lines, { taxRate: 0.21 }).total;
  return (
    <li>
      <button type="button" onClick={() => onSelect(order)}>
        {order.id} · {describeState(order.state)} · {formatCents(total)}
      </button>
    </li>
  );
});

export function OrderList({ customerId, onSelect }: OrderListProps) {
  const orders = useOrders(customerId);
  const [filter, setFilter] = useState('');

  const visible = useMemo(() => {
    if (orders.status !== 'ready') {
      return [];
    }
    const needle = filter.trim().toLowerCase();
    return orders.orders.filter((order) => order.id.toLowerCase().includes(needle));
  }, [orders, filter]);

  const handleFilter = useCallback((event: ChangeEvent<HTMLInputElement>) => setFilter(event.target.value), []);
  const handleSelect = useCallback((order: Order) => onSelect?.(order), [onSelect]);

  switch (orders.status) {
    case 'loading':
      return <p aria-busy="true">Loading orders…</p>;
    case 'error':
      return (
        <p role="alert">
          Could not load orders: {orders.error.message}{' '}
          <button type="button" onClick={orders.reload}>
            Retry
          </button>
        </p>
      );
    case 'ready':
      return (
        <section>
          <input value={filter} onChange={handleFilter} placeholder="Filter by order id" />
          {visible.length === 0 ? (
            <p>No orders match.</p>
          ) : (
            <ul>
              {visible.map((order) => (
                <OrderRow key={order.id} order={order} onSelect={handleSelect} />
              ))}
            </ul>
          )}
        </section>
      );
    default:
      return assertNever(orders);
  }
}
