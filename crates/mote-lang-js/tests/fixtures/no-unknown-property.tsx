import { useState } from 'react';

export function Search(): JSX.Element {
  const [query, setQuery] = useState('');
  return (
    <div>
      <input
        value={query}
        onchange={(e: any) => setQuery(e.target.value)} // expect: no-unknown-property
        autocomplete="off" // expect: no-unknown-property
      />
      <input value={query} onChange={(e) => setQuery(e.target.value)} autoComplete="off" />
    </div>
  );
}
