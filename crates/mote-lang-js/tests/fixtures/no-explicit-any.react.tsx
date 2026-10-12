export const Card = (props: any) => <div>{props.title}</div>; // expect: no-explicit-any
export const Typed = (props: { title: string }) => <div>{props.title}</div>;
