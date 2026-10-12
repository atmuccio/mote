type User = { id: number; admin: boolean };

export function Badge({ users }: { users: User[] }) {
  if (users.find((u: User) => u.admin) as User | undefined) { // expect: prefer-array-some
    return <b>admin</b>;
  }
  const first = users.find((u) => u.id === 1)!;
  return <span>{users.find((u) => u.admin) && <i>{first.id}</i>}</span>;
}

export function Toolbar({ items }: { items: { id: string; on: boolean }[] }) {
  const active = items.find((i) => i.on); // expect: prefer-array-some
  const selected = items.find((i) => i.on);
  const current = items.find((i) => i.on)!;
  if (!active) return null;
  return (
    <div>
      {selected && <span>{selected.id}</span>}
      {current.id}
    </div>
  );
}

export function guard(items: { on: boolean }[]): void {
  const hit = items.find((i) => i.on) as { on: boolean } | undefined; // expect: prefer-array-some
  if (!hit) throw new Error('none');
}
