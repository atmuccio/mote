export function List({ items }: { items: Item[] }) {
  let n = 0;
  return (
    <ul ref={(el) => (listRef.current = el)}>
      {items.map((item) => (
        <li key={item.id} data-n={n++}> {/* expect: assignment-in-operand */}
          {(item.label as string)}
        </li>
      ))}
    </ul>
  );
}

export function typed(map: Map<string, number>, key: string): number {
  return (map.get(key)! as number) + (total = 1); // expect: assignment-in-operand
}
