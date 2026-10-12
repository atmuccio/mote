export class Service {
  async start(): Promise<void> {}

  run(): void {
    this.start(); // not checked: method calls need type information
    bootstrap(); // expect: floating-promise
  }
}

async function bootstrap(): Promise<void> {
  await new Service().start();
}

bootstrap(); // expect: floating-promise
bootstrap().catch((error: unknown) => console.error(error));
void bootstrap();

declare function external(): Promise<void>;
external();

const typed: () => Promise<void> = async () => {};
typed(); // expect: floating-promise
