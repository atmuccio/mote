class HttpError extends Error {
  constructor(readonly status: number) {
    super(`HTTP ${status}`);
  }
}

export function handle(status: number): void {
  if (status >= 400) {
    new HttpError(status); // expect: exception-not-thrown
  }
  new HttpError(status) as unknown; // expect: exception-not-thrown
  (new HttpError(status))!; // expect: exception-not-thrown
}

export function fine(status: number): HttpError {
  if (status >= 500) {
    throw new HttpError(status);
  }
  const e: Error = new HttpError(status);
  errors.push(new HttpError(status));
  return e as HttpError;
}

declare const errors: Error[];
