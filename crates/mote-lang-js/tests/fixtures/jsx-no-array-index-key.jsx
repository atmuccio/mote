import React, { Children } from 'react';

export function List({ items, children }) {
  return (
    <div>
      {items.map((item, index) => <Row key={index} item={item} />)} {/* expect: jsx-no-array-index-key */}
      {items.map((item, i) => (
        <Row key={`row-${i}`} item={item} /> // expect: jsx-no-array-index-key
      ))}
      {items.map((item, i) => <Row key={'row-' + i} item={item} />)} {/* expect: jsx-no-array-index-key */}
      {items.map((item, i) => <Row key={String(i)} item={item} />)} {/* expect: jsx-no-array-index-key */}
      {items.map((item, i) => <Row key={i.toString()} item={item} />)} {/* expect: jsx-no-array-index-key */}
      {items.filter(Boolean).map(function (item, idx) {
        return <Row key={idx} item={item} />; // expect: jsx-no-array-index-key
      })}
      {items.reduce((acc, item, i) => [...acc, <Row key={i} item={item} />], [])} {/* expect: jsx-no-array-index-key */}
      {Children.map(children, (child, i) => <li key={i}>{child}</li>)} {/* expect: jsx-no-array-index-key */}
      {React.Children.map(children, (child, i) => <li key={i}>{child}</li>)} {/* expect: jsx-no-array-index-key */}
      {items?.map((item, i) => <Row key={i} item={item} />)} {/* expect: jsx-no-array-index-key */}
    </div>
  );
}

export function Compliant({ items, rows }) {
  return (
    <div>
      {items.map((item) => <Row key={item.id} item={item} />)}
      {items.map((item, i) => <Row key={`${item.id}-${i}`} item={item} />)}
      {items.map((item, i) => <Row key={item.id + i} item={item} />)}
      {Array.from({ length: 3 }, (_, i) => <Cell key={i} />)}
      {items.map((item, i) => <Row key={item.id} position={i} item={item} />)}
      {items.reduce((acc, item) => [...acc, <Row key={item.id} item={item} />], [])}
      {rows.map((row) => row.cells.map((cell, i) => <Cell key={row.id} index={i} />))}
      {items.map((item, i) => {
        const key = item.id;
        return <Row key={key} item={item} />;
      })}
      {items.map((item, i) => <Row key="static" item={item} />)}
    </div>
  );
}

function Row() {
  return null;
}
function Cell() {
  return null;
}

// The first parameter of a map callback is the item, not the index.
export function ItemKeys({ ids }) {
  return ids.map((id) => <Row key={id} />);
}

// A local variable named like an index is not a callback parameter.
export function Single() {
  const i = 0;
  return <Row key={i} />;
}
