import * as React from 'react';

interface Item { id: string; label: string }

export const Menu: React.FC<{ items: Item[] }> = ({ items }) => (
  <ul>
    {items.map((item) => <MenuItem label={item.label} /> as React.ReactElement)} {/* expect: jsx-key */}
    {items?.map((item) => <MenuItem key={item.id} label={item.label} />)}
    {items.map((item): JSX.Element => <MenuItem key={item.id} label={item.label} />)}
  </ul>
);
