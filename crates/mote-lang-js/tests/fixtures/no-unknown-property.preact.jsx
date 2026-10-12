import { h } from 'preact';
import { useState } from 'preact/hooks';

// Preact accepts HTML attribute names: nothing is reported here.
export function Form({ save }) {
  const [name] = useState('');
  return (
    <form>
      <label class="field" for="name">{name}</label>
      <button onclick={save}>Save</button>
    </form>
  );
}

export const unused = h;
