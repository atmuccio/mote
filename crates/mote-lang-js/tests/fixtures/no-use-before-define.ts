// Types and export lists are not runtime uses.
export { value };
let annotated: typeof value;
type Alias = typeof value;
const value = 1;
namespace Space {
  export const inner = helper(); // expect: no-use-before-define
  const helper = () => 1;
}
declare const ambient: number;
export default function f(): Alias {
  annotated = value;
  return annotated + ambient + Space.inner;
}
