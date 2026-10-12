type Path = 'a\/b'; // expect: no-useless-escape
export const pattern: RegExp = /\@user/i; // expect: no-useless-escape
export const ok: string = 'a/b\\c';
export const ok2 = /\//g;
enum Sep {
  Colon = '\:', // expect: no-useless-escape
  Slash = '/',
}
export { Sep };
export type { Path };
