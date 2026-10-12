export class Repo {
  async load(id: string): Promise<Item> {
    try {
      return await this.db.get(id);
    } catch (e: unknown) { // expect: no-useless-catch
      throw e;
    }
  }

  async loadOrDefault(id: string): Promise<Item | undefined> {
    try {
      return await this.db.get(id);
    } catch (e: unknown) {
      if (isNotFound(e)) {
        return undefined;
      }
      throw e;
    }
  }

  parse(text: string): Item {
    try {
      return JSON.parse(text) as Item;
    } catch (e) { // expect: no-useless-catch
      throw e as Error; // a type assertion does not change the value
    }
  }
}
