export class Service {
  constructor(private readonly name: string, public level: number) {
    level = 3;
    use(level, name);
  }

  handle(request: Request): Response {
    request = new Request('/fallback'); // expect: incoming-value-overwritten
    return fetchSync(request);
  }

  typed(value: unknown): string {
    value = String(value as string);
    return value as string;
  }
}
