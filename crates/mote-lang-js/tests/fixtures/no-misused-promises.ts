export class Gate {
  async canActivate(): Promise<boolean> {
    return true;
  }

  check(ids: string[]): void {
    const valid = ids.find(async (id: string) => this.canActivate()); // expect: no-misused-promises
    ids.every(async (id) => (await lookup(id)) !== undefined); // expect: no-misused-promises
    if (lookup(ids[0])) { // expect: no-misused-promises
      use(valid);
    }
    if (this.canActivate()) {
      use(ids);
    }
  }
}

async function lookup(id: string): Promise<string | undefined> {
  return id;
}

export const optional = (list?: string[]) => list?.filter(async (x) => x.length > 0); // expect: no-misused-promises
