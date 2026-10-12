interface Props { title: string }
const props = { title: 'x' } as Props; // expect: consistent-type-assertions
const node = document.body as HTMLElement;
export const View = () => <div title={(props as Props).title} />;
