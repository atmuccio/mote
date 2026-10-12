// No React evidence: another JSX runtime, not checked.
export function List({ items }) {
  return <ul>{items.map((item, i) => <li key={i}>{item}</li>)}</ul>;
}
