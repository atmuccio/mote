export function parse(input: string): string[] {
  const parts: string[] = input.split(/;\s+?/); // expect: regex-lazy-quantifier-at-end
  const head = (/^(\w+?)/ as RegExp).exec(input); // expect: regex-lazy-quantifier-at-end
  const ok = input.split(/;\s+?(?=\w)/);
  return [...parts, ...ok, String(head)];
}

class Scanner {
  private readonly pattern = /--.*?/;

  strip(line: string): string {
    return line.replace(this.pattern, '').replace(/\/\*.*?\*\//g, '');
  }
}

export const scanner = new Scanner();
