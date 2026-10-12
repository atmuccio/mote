enum Color { Red, Green }
function name(c: Color): string {
  switch (c) { // expect: default-case
    case Color.Red:
      return 'red';
    case Color.Green:
      return 'green';
  }
  return '';
}
