import { h } from 'preact';

function useState(initial) {
  return [initial, () => {}];
}

export function NotReact() {
  const [value, setValue] = useState(0);
  setValue(1);
  return <span>{value}</span>;
}

export function Flagged() {
  const [v, setV] = React.useState(0);
  setV(1); // expect: no-set-state-in-render
  return <span>{v}</span>;
}
