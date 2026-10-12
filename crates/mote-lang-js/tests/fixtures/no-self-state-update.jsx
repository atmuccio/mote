import React, { useState, useEffect } from 'react';

export function Positives({ item }) {
  const [items, setItems] = useState([]);
  const [user, setUser] = React.useState({ name: '' });

  function add() {
    items.push(item);
    setItems(items); // expect: no-self-state-update
  }

  function rename(name) {
    user.name = name;
    setUser(user); // expect: no-self-state-update
  }

  useEffect(() => {
    setItems(items); // expect: no-self-state-update
  }, []);

  return <button onClick={add} onBlur={rename}>{items.length}</button>;
}

// ---- compliant ----
export function Negatives({ item, next }) {
  const [items, setItems] = useState([]);
  const [count, setCount] = useState(0);

  function add() {
    setItems([...items, item]);
    setItems((prev) => [...prev, item]);
    setCount(count + 1);
    setCount(next);
    [1, 2].forEach((count) => setCount(count));
  }

  return <button onClick={add}>{items.length}</button>;
}

function useLocalState(initial) {
  return [initial, () => {}];
}

export function NotReact() {
  const [value, setValue] = useLocalState(1);
  setValue(value);
  return value;
}
