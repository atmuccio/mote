// Direct eval can read any local by name: the rule stays silent in this file.
export function dynamic(code) {
  const hidden = 1;
  return eval(code);
}
