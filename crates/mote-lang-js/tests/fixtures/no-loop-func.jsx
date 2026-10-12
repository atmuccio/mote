export function Rows({ items }) {
  const rows = [];
  for (var i = 0; i < items.length; i++) {
    rows.push(<Row key={items[i].id} onClick={() => select(i)} />); // expect: no-loop-func
  }
  for (let j = 0; j < items.length; j++) {
    rows.push(<Row key={items[j].id} onClick={() => select(j)} />);
  }
  for (var k = 0; k < items.length; k++) {
    rows.push(<Row key={items[k].id} render={() => items.length} label={String(k)} />);
  }
  return <ul>{rows}</ul>;
}
