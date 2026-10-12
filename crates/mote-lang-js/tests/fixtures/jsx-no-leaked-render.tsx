import React from 'react';

type Props = { items: string[]; count: number; open: boolean };

export function Positives({ items, open }: Props, count: number, total: number) {
  return (
    <div>
      {items.length && <List items={items} />} {/* expect: jsx-no-leaked-render */}
      {count && <Badge />} {/* expect: jsx-no-leaked-render */}
      {open && items.length && <List items={items} />} {/* expect: jsx-no-leaked-render */}
      {total - count && <Badge />} {/* expect: jsx-no-leaked-render */}
      {items.length && items.map((item) => <li key={item}>{item}</li>)} {/* expect: jsx-no-leaked-render */}
    </div>
  );
}

export function Returned({ items }: Props) {
  return items.length && <List items={items} />; // expect: jsx-no-leaked-render
}

export const Arrow = ({ items }: Props) => items.length && <List items={items} />; // expect: jsx-no-leaked-render

// ---- compliant ----
export function Negatives({ items, open }: Props, count: number, name: string) {
  return (
    <div title={String(items.length && 'x')}>
      {items.length > 0 && <List items={items} />}
      {!!count && <Badge />}
      {open && <Badge />}
      {name && <Badge />}
      {count ? <Badge /> : null}
      {items.length && 'text'}
      <Badge data-count={count && 1} />
    </div>
  );
}

export function helper(items: string[]) {
  const size = items.length && items[0];
  return size;
}

function List({ items }: { items: string[] }) {
  return <ul>{items.map((i) => <li key={i}>{i}</li>)}</ul>;
}

function Badge() {
  return <span />;
}
