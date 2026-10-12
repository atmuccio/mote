export function check(value: string | undefined): void {
  if (value) {
    if (value.length > 3) { // expect: collapsible-if
      use(value as string);
    }
  }
}
