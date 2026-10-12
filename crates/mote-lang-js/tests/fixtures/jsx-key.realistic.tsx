import React from 'react';

interface Column<T> { key: keyof T; title: string; render?: (row: T) => React.ReactNode }
interface Props<T> { rows: T[]; columns: Column<T>[]; groups: Record<string, T[]> }

export function DataTable<T extends { id: string }>({ rows, columns, groups }: Props<T>) {
  const headers = columns.map((c) => <th key={String(c.key)}>{c.title}</th>);
  const cells = (row: T) => columns.map((c) => (
    <td key={String(c.key)}>{c.render ? c.render(row) : String(row[c.key])}</td>
  ));
  const decorated = rows.map((r) => ({ ...r, badge: <b>{r.id}</b> }));
  const pairs = rows.map((r) => [r.id, <Cell value={r.id} />] as const);
  return (
    <table>
      <thead><tr>{headers}</tr></thead>
      <tbody>
        {rows.map((row) => <tr key={row.id}>{cells(row)}</tr>)}
        {Object.entries(groups).map(([name, items]) => (
          <React.Fragment key={name}>
            <tr><th colSpan={columns.length}>{name}</th></tr>
            {items.map((item) => (
              <tr key={item.id}>{cells(item)}</tr>
            ))}
          </React.Fragment>
        ))}
        {rows.map((row) => (
          <tr> {/* expect: jsx-key */}
            {cells(row)}
          </tr>
        ))}
      </tbody>
      <tfoot>{decorated.length}{pairs.length}</tfoot>
    </table>
  );
}

declare function Cell(props: { value: string }): JSX.Element;
