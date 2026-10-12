import React, { memo, forwardRef, useCallback, useMemo } from 'react';

export function List({ items }) {
  function Row({ item }) { // expect: no-nested-component-definition
    return <li>{item.name}</li>;
  }
  const Header = () => <h1>Items</h1>; // expect: no-nested-component-definition
  const Footer = function () { // expect: no-nested-component-definition
    return items.length ? <footer>{items.length}</footer> : null;
  };
  return (
    <ul>
      <Header />
      {items.map((item) => <Row key={item.id} item={item} />)}
      <Footer />
    </ul>
  );
}

export const Table = ({ rows }) => {
  const Cell = ({ value }) => <td>{value}</td>; // expect: no-nested-component-definition
  return <Grid rows={rows} cell={Cell} />;
};

export const Memoized = memo(function Memoized() {
  const Inner = () => <span />; // expect: no-nested-component-definition
  return <Inner />;
});

export const WithRef = forwardRef((props, ref) => {
  const Inner = () => <span ref={ref} />; // expect: no-nested-component-definition
  return <Inner />;
});

// Compliant: helpers only called as functions.
export function Page({ user }) {
  const Title = () => <h1>{user.name}</h1>;
  function renderBody() {
    return <p>{user.bio}</p>;
  }
  return (
    <main>
      {Title()}
      {renderBody()}
    </main>
  );
}

// Compliant: inside hooks.
export function Hooks({ label }) {
  const Label = useMemo(() => () => <b>{label}</b>, [label]);
  const render = useCallback(() => {
    const Inner = () => <i>{label}</i>;
    return <Inner />;
  }, [label]);
  return <Label render={render} />;
}

// Compliant: a lower-case outer function is not a component.
function makeComponent(label) {
  const Generated = () => <b>{label}</b>;
  return Generated;
}

// Compliant: nested values that are not components.
export function Form() {
  const Validator = () => true;
  const Schema = { name: 'x' };
  return <form data-valid={Validator()} data-schema={Schema.name} />;
}

// Compliant: module-level components.
function Grid({ rows, cell: CellComponent }) {
  return rows.map((row) => <CellComponent key={row.id} value={row.value} />);
}
const Top = () => <div />;
export { makeComponent, Top };
