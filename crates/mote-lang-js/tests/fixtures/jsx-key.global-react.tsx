// Uses the global (UMD) React.
export const List: React.FC<{ items: string[] }> = ({ items }) => (
  <ul>{items.map((i) => <li>{i}</li>)}</ul> // expect: jsx-key
);
