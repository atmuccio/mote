// Solid does not use keys.
import { For } from 'solid-js';

export const List = (props: { items: string[] }) => (
  <ul>{props.items.map((item) => <li>{item}</li>)}</ul>
);
