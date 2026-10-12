export class Theme { // expect: no-extraneous-class
  static primary = 'blue';
}
export const Box = () => <div style={{ color: Theme.primary }} />;
