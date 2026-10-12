export function isValid(text: string) {
  if (!text) {
    return; // expect: consistent-return
  }
  return (JSON.parse(text) as Config).valid === true;
}

export function maybe(text: string): boolean | undefined {
  if (!text) return;
  return text.length > 2;
}

export async function saved(item: Item): Promise<void | boolean> {
  if (!item) return;
  return false;
}

export function typed(x: string): boolean {
  if (!x) return; // expect: consistent-return
  return x !== 'no';
}

export abstract class Handler {
  abstract handles(x: string): boolean;
  accepts(x: string) {
    if (!x) return; // expect: consistent-return
    return !this.handles(x);
  }
}
