export function isInsiders(appName: string): boolean {
  return appName.indexOf('Insider') > 0; // expect: index-of-positive-check
}

export function hasExtension(name: string): boolean {
  return name.indexOf('.') > 0; // deliberate: a dot after a non-empty base name
}

export function ok(names: readonly string[], name: string): boolean {
  return (names as string[]).indexOf(name) !== -1 && names.indexOf(name, 2) > 0;
}

export function wrapped(names: string[], name: string): boolean {
  return names.indexOf(name)! > 0; // expect: index-of-positive-check
}
