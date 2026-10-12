interface User { name: string; friend?: User }
declare const user: User | undefined;
declare const map: Map<string, User>;

const a = user!.name; // expect: no-non-null-assertion
const b = map.get('x')!; // expect: no-non-null-assertion
const c = user!!.name; // expect: no-non-null-assertion
const d = user?.friend!.name; // expect: no-non-null-assertion
let e: User | undefined;
e! = { name: 'x' }; // expect: no-non-null-assertion
const f = (user as User).name;
const g = user?.name ?? 'anonymous';
const h = !user;
const i = a != b;
const j = a !== b;
function k(u?: User) {
  if (!u) {
    return '';
  }
  return u.name;
}
const l = [user].filter(Boolean)
  .map((u) => u!.name); // expect: no-non-null-assertion
