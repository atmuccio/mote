let value: string | number = 1;
value = value as string;
value = value!;
value = value; // expect: no-self-assign
export class Store {
  private items: string[] = [];
  @observable accessor watched = 0;
  @reactive tracked = 0;
  reset(): void {
    this.items = this.items; // expect: no-self-assign
    this.items = [...this.items];
    this.watched = this.watched;
    this.tracked = this.tracked;
  }
}
declare function observable(...args: unknown[]): any;
declare function reactive(...args: unknown[]): any;
