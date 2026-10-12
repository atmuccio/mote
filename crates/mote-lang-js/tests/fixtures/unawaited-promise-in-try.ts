function request(url: string): Promise<string> {
  return fetch(url).then((r) => r.text());
}

const parse = (text: string): Promise<object> => Promise.resolve(JSON.parse(text));

export class Client {
  private async send(body: string): Promise<void> {
    await request(body);
  }

  private field = async (x: number): Promise<number> => x;

  get(url: string): Promise<string> {
    return request(url);
  }

  async load(url: string): Promise<string> {
    try {
      return request(url); // expect: unawaited-promise-in-try
    } catch (error: unknown) {
      return '';
    }
  }

  async post(body: string): Promise<void> {
    try {
      this.send(body);
      return this.send(body) as Promise<void>; // expect: unawaited-promise-in-try
    } catch {
      return;
    }
  }

  async viaField(): Promise<number> {
    try {
      return this.field(1); // expect: unawaited-promise-in-try
    } catch {
      return 0;
    }
  }

  async viaGetter(url: string): Promise<string> {
    try {
      return this.get(url); // expect: unawaited-promise-in-try
    } catch {
      return '';
    }
  }

  async parsed(text: string): Promise<object> {
    try {
      return parse(text); // expect: unawaited-promise-in-try
    } catch {
      return {};
    }
  }

  async unknownMethod(): Promise<number> {
    try {
      return this.compute();
    } catch {
      return 0;
    }
  }

  compute(): number {
    return 1;
  }

  async optional(url?: string): Promise<string | undefined> {
    try {
      return await request(url ?? '');
    } catch {
      return undefined;
    }
  }
}

// A same-file exported async function whose whole body is a try/catch does not reject, even when
// its return type is written out.
export async function safeSend(body: string): Promise<number> {
  try {
    await request(body);
    return 1;
  } catch (error) {
    return 0;
  }
}

export async function riskySend(body: string): Promise<number> {
  await request(body);
  return 1;
}

export function plainSend(body: string): Promise<number> {
  try {
    return request(body).then(() => 1);
  } catch (error) {
    return Promise.resolve(0);
  }
}

export async function sendAll(body: string): Promise<number> {
  try {
    if (!body) {
      return safeSend('empty');
    }
    return riskySend(body); // expect: unawaited-promise-in-try
  } catch (error) {
    return -1;
  }
}

export async function sendPlain(body: string): Promise<number> {
  try {
    return plainSend(body); // expect: unawaited-promise-in-try
  } catch (error) {
    return -1;
  }
}

// ---- retries, setup statements, resolve-only promises (typed) ----

const fetchPage = async (url: string, attempt: number = 0): Promise<Response | null> => {
  await warmUp();
  try {
    const res = await fetch(url);
    if (res.status === 429 && attempt < 3) {
      return fetchPage(url, attempt + 1);
    }
    return res;
  } catch (error: unknown) {
    return attempt < 3 ? fetchPage(url, attempt + 1) : null;
  }
};

interface Status {
  name: string;
  ok: boolean;
}

async function probe(name: string, retries = 2): Promise<Status> {
  if (!name.includes('/')) {
    return { name, ok: false } satisfies Status;
  }
  const headers: Record<string, string> = {};
  try {
    const res = await fetch(`/repos/${name}`, { headers });
    if (res.status === 403 && retries > 0) {
      await new Promise<void>((resolve) => setTimeout(resolve, 1000));
      return probe(name, retries - 1);
    }
    return { name, ok: res.ok };
  } catch (error) {
    return { name, ok: false };
  }
}

export async function probeAll(names: string[]): Promise<Status | undefined> {
  try {
    return probe(names[0]);
  } catch {
    return undefined;
  }
}

export async function connect(url: string): Promise<boolean> {
  try {
    const socket = new WebSocket(url);
    return new Promise<boolean>((resolve) => {
      socket.addEventListener('open', () => resolve(true));
      socket.addEventListener('error', () => resolve(false));
    });
  } catch {
    return false;
  }
}

export async function connectOrFail(url: string): Promise<boolean> {
  try {
    return new Promise<boolean>((resolve, reject) => { // expect: unawaited-promise-in-try
      const socket = new WebSocket(url);
      socket.addEventListener('open', () => resolve(true));
      socket.addEventListener('error', reject);
    });
  } catch {
    return false;
  }
}
