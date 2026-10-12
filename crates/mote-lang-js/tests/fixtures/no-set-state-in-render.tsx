import { useState } from 'react';

type Props = { initial: number };

export function Typed({ initial }: Props) {
  const [value, setValue] = useState<number>(0);
  setValue(initial); // expect: no-set-state-in-render
  return <span>{value}</span>;
}

export const Guarded = ({ initial }: Props) => {
  const [value, setValue] = useState<number | null>(null);
  if (value === null) setValue(initial);
  return <span>{value}</span>;
};

export function Optional({ initial }: Props) {
  const [value, setValue] = useState(initial);
  const update = (n: number) => setValue(n);
  return <input value={value} onChange={(e) => update(Number(e.target.value))} />;
}
