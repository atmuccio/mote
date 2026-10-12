export function check(enabled: boolean, maybe?: boolean, loose: boolean | undefined = undefined): void {
  if (enabled === true) {} // expect: no-redundant-boolean
  if (maybe === true) {}
  if (loose === false) {}
  const typed: boolean = compute();
  if (typed !== false) {} // expect: no-redundant-boolean
  let uninit: boolean;
  const flag = enabled as boolean;
  if (flag === true) {}
}
const withDefault = (on: boolean = false) => on === true; // expect: no-redundant-boolean
export const migrate = (value: boolean) => {
  if (value === true) {
    return 'on';
  } else if (value === false) {
    return 'off';
  }
  return value;
};
export function nullable(flag: boolean): string {
  if (flag === null) {
    return 'unset';
  }
  return flag === true ? 'yes' : 'no';
}
