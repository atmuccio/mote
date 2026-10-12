export const List = ({ items }: { items: string[] }) => (
  <ul>{items.map(function (item) { return <li key={item}>{item}</li>; })}</ul> // expect: prefer-arrow-callback
);
