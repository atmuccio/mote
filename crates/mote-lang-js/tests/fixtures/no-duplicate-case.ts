enum Color { Red, Green }
export function name(c: Color): string {
  switch (c) {
    case Color.Red:
      return 'red';
    case Color.Green:
      return 'green';
    case Color.Red: // expect: no-duplicate-case
      return 'again';
    case (Color.Green as Color):
      return 'cast';
  }
  return '';
}
