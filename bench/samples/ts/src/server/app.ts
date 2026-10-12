import { createServer } from 'node:http';
import { loadConfig } from './config';
import { createRoutes, dispatch } from './http/router';
import { InMemoryOrderRepository } from './orders/orderRepository';
import { OrderService } from './orders/orderService';

export async function main(): Promise<void> {
  const config = loadConfig();
  const service = new OrderService(new InMemoryOrderRepository());
  const routes = createRoutes(service);

  const server = createServer((req, res) => {
    dispatch(routes, req, res).catch((error: unknown) => {
      process.stderr.write(`unhandled error: ${String(error)}\n`);
      res.statusCode = 500;
      res.end();
    });
  });

  await new Promise<void>((resolve) => server.listen(config.port, config.host, resolve));
  // mote-ignore[js/no-console]: the startup banner on stdout is what the process supervisor waits for
  console.log(`orders-service listening on http://${config.host}:${config.port}`);

  const shutdown = (signal: NodeJS.Signals) => {
    process.stderr.write(`received ${signal}, shutting down\n`);
    server.close(() => process.exit(0));
  };
  process.once('SIGINT', shutdown);
  process.once('SIGTERM', shutdown);
}

main().catch((error: unknown) => {
  process.stderr.write(`fatal: ${error instanceof Error ? error.stack : String(error)}\n`);
  process.exitCode = 1;
});
