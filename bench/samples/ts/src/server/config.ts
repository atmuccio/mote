import { readFileSync } from 'node:fs';

export interface ServiceConfig {
  readonly port: number;
  readonly host: string;
  readonly databaseUrl: string;
  readonly logLevel: 'debug' | 'info' | 'warn' | 'error';
  readonly features: ReadonlySet<string>;
}

const LOG_LEVELS = ['debug', 'info', 'warn', 'error'] as const;

function isLogLevel(value: string): value is ServiceConfig['logLevel'] {
  return (LOG_LEVELS as readonly string[]).includes(value);
}

/** Reads the configuration from the environment, with safe defaults for local development. */
export function loadConfig(env: NodeJS.ProcessEnv = process.env): ServiceConfig {
  const level = env.LOG_LEVEL?.toLowerCase() ?? 'info';
  const port = Number.parseInt(env.PORT ?? '', 10);
  return {
    port: Number.isNaN(port) ? 8080 : port,
    host: env.HOST ?? '127.0.0.1',
    databaseUrl: env.DATABASE_URL ?? 'postgres://localhost:5432/orders',
    logLevel: isLogLevel(level) ? level : 'info',
    features: new Set((env.FEATURES ?? '').split(',').filter(Boolean)),
  };
}

export function readSecret(path: string): string | undefined {
  try {
    return readFileSync(path, 'utf8').trim();
  } catch {
    // A missing secrets file is normal outside production; the caller falls back to env vars.
    return undefined;
  }
}

export function describe(config: ServiceConfig): string {
  // smell (rule enabled in .mote.yaml): concatenation instead of a template literal.
  return 'listening on ' + config.host + ':' + config.port;
}

export function retryBudget(config: ServiceConfig): number {
  // smell (rule enabled in .mote.yaml): the annotation repeats the inferred type.
  const retries: number = 3;
  let budget = retries * 1000;
  if (config.features.has('fast-retry')) {
    try {
      budget = Number(readSecret('/run/secrets/retry-budget'));
    } catch (error) {}
  }
  return budget;
}

export function dumpConfig(config: ServiceConfig): string {
  // smell: `let` for a value that is never reassigned.
  let keys = Object.keys(config).sort();
  // bug: a constant condition around a leftover debugger statement.
  if (false) {
    debugger;
  }
  return keys.join(',');
}
