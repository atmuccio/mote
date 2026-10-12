export function parse(text: string): Config {
  const config: Config = JSON.parse(text);
  return config;
}

export function cast(text: string): Config {
  // expect-next-line: immediately-returned-variable
  const config = JSON.parse(text) as Config;
  return config;
}

export function disposable(): Resource {
  using res = open();
  return res;
}

declare function ambient(): void;
