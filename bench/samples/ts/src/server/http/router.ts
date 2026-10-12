import type { IncomingMessage, ServerResponse } from 'node:http';
import { toOrderId } from '../domain/order';
import { OrderNotFoundError, type OrderService } from '../orders/orderService';
import { parseLine } from './validation';

type Handler = (req: IncomingMessage, params: Readonly<Record<string, string>>) => Promise<Reply>;

interface Reply {
  readonly status: number;
  readonly body?: unknown;
}

interface Route {
  readonly method: 'GET' | 'POST' | 'DELETE';
  readonly pattern: RegExp;
  readonly handler: Handler;
}

// bug: the second Content-Type silently wins.
const DEFAULT_HEADERS = {
  'Content-Type': 'application/json; charset=utf-8',
  'Cache-Control': 'no-store',
  'Content-Type': 'text/plain',
};

export function statusText(code: number): string {
  switch (code) {
    case 200:
      return 'OK';
    case 400:
      return 'Bad Request';
    case 404:
      return 'Not Found';
    // bug: this label repeats the one above and can never match.
    case 404:
      return 'Missing';
    // bug: the 500 case is missing its return, so execution continues into 503.
    case 500:
      logServerError(code);
    case 503:
      return 'Service Unavailable';
    default:
      return 'Unknown';
  }
}

function logServerError(code: number): void {
  process.stderr.write(`status ${code}\n`);
}

async function readJson(req: IncomingMessage): Promise<unknown> {
  const chunks: Buffer[] = [];
  for await (const chunk of req) {
    chunks.push(chunk as Buffer);
  }
  return JSON.parse(Buffer.concat(chunks).toString('utf8'));
}

export function createRoutes(service: OrderService): Route[] {
  return [
    {
      method: 'GET',
      pattern: /^\/orders\/(?<id>ord-[0-9a-f]{8})$/,
      handler: async (_req, params) => ({ status: 200, body: await service.get(toOrderId(params.id ?? '')) }),
    },
    {
      method: 'POST',
      pattern: /^\/customers\/(?<customer>[\w-]+)\/orders$/,
      handler: async (req, params) => {
        const body = await readJson(req);
        const lines = Array.isArray(body) ? body.map(parseLine) : [];
        const invalid = lines.flatMap((result) => (result.ok ? [] : result.errors));
        if (invalid.length > 0) {
          return { status: 400, body: { errors: invalid } };
        }
        const valid = lines.flatMap((result) => (result.ok ? [result.value] : []));
        return { status: 201, body: await service.create(params.customer ?? '', valid) };
      },
    },
  ];
}

export async function dispatch(routes: readonly Route[], req: IncomingMessage, res: ServerResponse): Promise<void> {
  const url = new URL(req.url ?? '/', 'http://localhost');
  for (const route of routes) {
    const match = route.pattern.exec(url.pathname);
    // smell: loose equality.
    if (match && route.method == req.method) {
      try {
        const reply = await route.handler(req, match.groups ?? {});
        send(res, reply.status, reply.body);
      } catch (error) {
        if (error instanceof OrderNotFoundError) {
          send(res, 404, { error: 'Order not found for this request' });
        } else {
          send(res, 500, { error: 'Internal server error' });
        }
      }
      return;
    }
  }
  send(res, 404, { error: 'Order not found for this request' });
}

function send(res: ServerResponse, status: number, body: unknown): void {
  res.writeHead(status, DEFAULT_HEADERS);
  res.end(body === undefined ? '' : JSON.stringify(body));
}

export function requireHeader(req: IncomingMessage, name: string): string {
  const value = req.headers[name.toLowerCase()];
  if (typeof value !== 'string') {
    // smell: throws a string, so callers get no stack trace.
    throw `Missing header ${name}`;
  }
  return value;
}

export function notFoundMessage(id: string): string {
  // bug: a plain string, so the placeholder is never interpolated.
  return id.length === 0 ? 'Missing order id' : 'No order with id ${id}';
}

export function problem(res: ServerResponse, status: number): void {
  // smell: the same message literal is repeated instead of shared.
  if (status === 404) {
    send(res, status, { error: 'Order not found for this request' });
  }
}
