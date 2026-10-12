import { useEffect, useState } from 'react';
import type { Order } from '../server/domain/order';
import { formatCents, priceLines } from '../server/orders/pricing';

interface OrderSummaryProps {
  readonly order?: Order;
  readonly onCancel: (order: Order) => Promise<void>;
}

export function OrderSummary({ order, onCancel }: OrderSummaryProps) {
  if (!order) {
    return <p>Select an order.</p>;
  }

  // bug: hooks called after an early return change the hook order between renders.
  const [expanded, setExpanded] = useState(false);
  useEffect(() => {
    document.title = `Order ${order.id}`;
  }, [order.id]);

  const breakdown = priceLines(order.lines, { taxRate: 0.21 });

  async function handleCancel(current: Order) {
    // smell: blocking browser dialog.
    if (confirm(`Cancel order ${current.id}?`)) {
      await onCancel(current);
    }
  }

  return (
    <article className="order-summary">
      <h2>{order.id}</h2>
      {/* bug: className is given twice; the first value is silently dropped. */}
      <button className="toggle" type="button" className="link" onClick={() => setExpanded(!expanded)}>
        {expanded ? 'Hide lines' : 'Show lines'}
      </button>
      {expanded && (
        <ul>
          {/* bug: list items without a key. */}
          {order.lines.map((line) => (
            <li>
              {line.quantity} × {line.sku} at {formatCents(line.unitPriceCents)}
            </li>
          ))}
        </ul>
      )}
      <p>Total: {formatCents(breakdown.total)}</p>
      <button type="button" onClick={() => void handleCancel(order)}>
        Cancel order
      </button>
    </article>
  );
}
