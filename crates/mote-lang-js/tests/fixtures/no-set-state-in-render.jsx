import React, { useState, useReducer, useEffect, Component } from 'react';

export function Counter({ start }) {
  const [count, setCount] = useState(0);
  setCount(start); // expect: no-set-state-in-render
  return <span>{count}</span>;
}

export const Toggle = ({ on }) => {
  const [state, dispatch] = useReducer(reducer, {});
  dispatch({ type: 'set', on }); // expect: no-set-state-in-render
  return <div>{String(state.on)}</div>;
};

export function Namespaced() {
  const [value, setValue] = React.useState('');
  setValue('x'); // expect: no-set-state-in-render
  return <input value={value} />;
}

export function useSelection(items) {
  const [selected, setSelected] = useState(null);
  setSelected(items[0]); // expect: no-set-state-in-render
  return selected;
}

export class Legacy extends Component {
  render() {
    this.setState({ rendered: true }); // expect: no-set-state-in-render
    return <p>{this.state.rendered}</p>;
  }
}

// ---- compliant ----

export function PropSync({ value }) {
  const [prev, setPrev] = useState(value);
  if (prev !== value) {
    setPrev(value);
  }
  return <span>{prev}</span>;
}

export function Handler() {
  const [count, setCount] = useState(0);
  const increment = () => {
    setCount(count + 1);
  };
  useEffect(() => {
    setCount(1);
  }, []);
  return <button onClick={increment}>{count}</button>;
}

export function ShortCircuit({ reset }) {
  const [n, setN] = useState(0);
  reset && setN(0);
  return <span>{n}</span>;
}

export function PassedDown({ setParent }) {
  setParent(1);
  return null;
}

export function Nested() {
  const [open, setOpen] = useState(false);
  function close() {
    setOpen(false);
  }
  return <Dialog open={open} onClose={close} />;
}

export class Other extends Component {
  componentDidMount() {
    this.setState({ ready: true });
  }

  render() {
    return <p>{this.state.ready}</p>;
  }
}

export function FirstElement() {
  const [setter] = useState(() => noop);
  setter();
  return null;
}

export function Loop({ items }) {
  const [, setLast] = useState(null);
  for (const item of items) {
    setLast(item);
  }
  return null;
}
