throw 'typed' as unknown as Error; // expect: no-throw-literal
throw error as Error;
throw new CancellationError();
