export function List({ items }: { items: string[] }) {
  if (!items.length) return null;
  if (items.length > 10) // expect: curly
    items = items.slice(0, 10);
  for (const item of items) console.log(item); // expect: curly
  return <ul>{items.map((i) => <li key={i}>{i}</li>)}</ul>;
}
