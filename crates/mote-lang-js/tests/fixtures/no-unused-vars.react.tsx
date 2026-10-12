import React from 'react';
import { Button } from './button';
import { Unused } from './unused'; // expect: no-unused-vars

export function App({ title, onClick }: { title: string; onClick: () => void }) {
  const Local = () => <span />;
  const unusedElement = <div />; // expect: no-unused-vars
  return <Button><Local />{title}</Button>;
}

export const Member = () => {
  const Parts = { Item: () => <li /> };
  return <Parts.Item />;
};
