function handle(key, action) {
  switch (key) {
    case 'Enter', 'Space': // expect: no-compound-case-label
      submit();
      break;
    case 'Up' || 'Down': // expect: no-compound-case-label
      move();
      break;
    case KEY_A || KEY_B: // expect: no-compound-case-label
      press();
      break;
    case Keys.Left || Keys.Right || Keys.Home: // expect: no-compound-case-label
      scroll();
      break;
    case 1 && 2: // expect: no-compound-case-label
      break;
    default:
      break;
  }
  // Compliant.
  switch (true) {
    case key === 'a' || key === 'b':
      break;
    case 'x' || 'y':
      break;
  }
  switch (action) {
    case 'Enter':
    case 'Space':
      submit();
      break;
    case prefix || 'default':
      break;
    case getKey() || fallback:
      break;
  }
}
export { handle };
