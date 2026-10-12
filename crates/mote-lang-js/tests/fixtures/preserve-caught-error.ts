export function parse(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch (error: unknown) {
    throw new SyntaxError('bad JSON'); // expect: preserve-caught-error
  }
}

export function parseOk(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch (error: unknown) {
    throw new SyntaxError('bad JSON', { cause: error as Error });
  }
}

export class Loader {
  load(text: string): unknown {
    try {
      return JSON.parse(text);
    } catch (e) {
      throw new Error(`Cannot load ${text}`) as Error; // expect: preserve-caught-error
    }
  }
}
