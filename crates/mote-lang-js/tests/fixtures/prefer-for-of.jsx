export function Rows({ rows }) {
  const out = [];
  for (let i = 0; i < rows.length; i++) { // expect: prefer-for-of
    out.push(<tr key={rows[i].id} />);
  }
  for (let i = 0; i < rows.length; i++) {
    out.push(<tr key={i}>{rows[i].name}</tr>);
  }
  return out;
}
