export function List({ items }: { items: string[] }) {
  return (
    <ul>
      {items.map((item) => { // expect: array-callback-return
        <li>{item}</li>;
      })}
      {items.map((item) => {
        return <li key={item}>{item}</li>;
      })}
    </ul>
  );
}
export async function flushAll(appenders: { flush(): Promise<void> }[]): Promise<void> {
  await Promise.all(appenders.map((a) => { // expect: array-callback-return
    void a.flush();
  }));
}
