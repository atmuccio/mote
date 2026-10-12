import { readFile as readFile } from 'fs'; // expect: no-useless-rename
import { writeFile as write, stat } from 'fs';
import def, { open as open } from 'fs'; // expect: no-useless-rename
import { 'string-name' as stringName } from 'mod';
export { readFile as readFile }; // expect: no-useless-rename
export { write as writeFile, stat };
export { x as x } from 'other'; // expect: no-useless-rename
export { y } from 'other';
const { id: id, name } = user; // expect: no-useless-rename
const { a: b } = user;
const { c } = user;
const { d: d = 1 } = user; // expect: no-useless-rename
const { ['e']: e } = user;
const { 'f': f } = user;
const { g: { g } } = user;
({ h: h } = user); // expect: no-useless-rename
({ i: i = 2 } = user); // expect: no-useless-rename
({ j } = user);
({ k: other.k } = user);
function params({ p: p }) { // expect: no-useless-rename
  return p;
}
const obj = { id: id };
