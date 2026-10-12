export function load(): string {
  try {
    return read();
  } catch (error: unknown) {
    error = String(error); // expect: no-ex-assign
    return error as string;
  }
}
declare function read(): string;
