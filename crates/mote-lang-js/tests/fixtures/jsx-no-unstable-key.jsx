import React from 'react';
import { v4 as uuid } from 'uuid';
import * as uuidLib from 'uuid';
import { nanoid } from 'nanoid';
import { uniqueId } from 'lodash';
import { validate } from 'uuid';

export function List({ items }) {
  return (
    <ul>
      {items.map((item) => <Row key={Math.random()} item={item} />)} {/* expect: jsx-no-unstable-key */}
      {items.map((item) => <Row key={uuid()} item={item} />)} {/* expect: jsx-no-unstable-key */}
      {items.map((item) => <Row key={`${item.name}-${nanoid()}`} item={item} />)} {/* expect: jsx-no-unstable-key */}
      {items.map((item) => <Row key={uuidLib.v4()} item={item} />)} {/* expect: jsx-no-unstable-key */}
      {items.map((item) => <Row key={uniqueId('row')} item={item} />)} {/* expect: jsx-no-unstable-key */}
      {items.map((item) => <Row key={Date.now() + item.id} item={item} />)} {/* expect: jsx-no-unstable-key */}
      {items.map((item) => <Row key={crypto.randomUUID()} item={item} />)} {/* expect: jsx-no-unstable-key */}
      {items.map((item) => <Row key={new Date().getTime()} item={item} />)} {/* expect: jsx-no-unstable-key */}
    </ul>
  );
}

// ---- compliant ----
export function Stable({ items, seed }) {
  const ids = React.useMemo(() => items.map(() => uuid()), [items]);
  return (
    <ul>
      {items.map((item) => <Row key={item.id} item={item} />)}
      {items.map((item, i) => <Row key={ids[i]} item={item} />)}
      {items.map((item) => <Row key={validate(item.id) ? item.id : item.name} item={item} />)}
      {items.map((item) => <Row key={new Date(item.created).getTime()} item={item} />)}
      {items.map((item) => <Row key={item.id} onClick={() => Math.random()} />)}
      <Chart key={seed} data={items} />
    </ul>
  );
}

function Math2() {
  const Math = { random: () => 1 };
  return <div key={Math.random()} />;
}
export { Math2 };
