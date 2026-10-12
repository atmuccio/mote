export const List = ({ items }: { items: Array<string> }) => <ul>{items.length}</ul>; // expect: array-type
export const Ok = ({ items }: { items: string[] }) => <ul>{items.length}</ul>;
