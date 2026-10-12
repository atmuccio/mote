// mote-test: params quoted_message=false

export function quotedAccepted(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch (error: unknown) {
    throw new Error(`bad JSON: ${(error as Error).message}`);
  }
}

export function stillReported(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch (error: unknown) {
    throw new Error('bad JSON'); // expect: preserve-caught-error
  }
}

export function optionsStillChecked(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error('bad JSON', { cause: 'PARSE' }); // expect: preserve-caught-error
  }
}

export class Loader {
  load(text: string): unknown {
    try {
      return JSON.parse(text);
    } catch (e) {
      throw new Error(String(e)) as Error;
    }
  }
}
