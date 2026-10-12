type Greeting = 'Hello ${name}';
interface Thing { id: string }
export function describe(id: string, thing: Thing): string {
  const label: 'Thing ${Thing}' = 'Thing ${Thing}';
  return 'Thing ${id}'; // expect: no-template-curly-in-string
}
