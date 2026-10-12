import { useCallback, useEffect, useState } from 'react';
import type { Order } from '../server/domain/order';

export type OrdersState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly error: Error }
  | { readonly status: 'ready'; readonly orders: readonly Order[] };

/** Loads a customer's orders; aborts the request when the customer changes or the view unmounts. */
export function useOrders(customerId: string): OrdersState & { reload: () => void } {
  const [state, setState] = useState<OrdersState>({ status: 'loading' });
  const [generation, setGeneration] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    setState({ status: 'loading' });

    async function load(): Promise<void> {
      try {
        const response = await fetch(`/api/customers/${encodeURIComponent(customerId)}/orders`, {
          signal: controller.signal,
        });
        if (!response.ok) {
          throw new Error(`HTTP ${response.status} while loading orders`);
        }
        const orders = (await response.json()) as Order[];
        setState({ status: 'ready', orders });
      } catch (error) {
        if (!controller.signal.aborted) {
          setState({ status: 'error', error: error instanceof Error ? error : new Error(String(error)) });
        }
      }
    }

    void load();
    return () => controller.abort();
  }, [customerId, generation]);

  const reload = useCallback(() => setGeneration((n) => n + 1), []);
  return { ...state, reload };
}
