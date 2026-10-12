export async function fetchAll(urls: string[]): Promise<string[]> {
  const out: string[] = [];
  try {
    for (const url of urls) {
      out.push(await get(url));
    }
  } finally {
    if (out.length === 0) {
      return []; // expect: no-unsafe-finally
    }
  }
  return out;
}
declare function get(url: string): Promise<string>;
