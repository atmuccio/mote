export function isReady(state: State): boolean { // expect: invariant-return
  if (state.loading) {
    return true as boolean;
  }
  return true;
}

export const code = (e: Err): number => { // expect: invariant-return
  if (e.fatal) return -1;
  return -1;
};

export function mixed(e: Err): number {
  if (e.fatal) return -1;
  return 1;
}

export function overloaded(a: string): number;
export function overloaded(a: number): number;
export function overloaded(a: any): number {
  if (a) return 1;
  return 2;
}
