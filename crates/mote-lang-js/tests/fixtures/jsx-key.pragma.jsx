/** @jsx React.createElement */
export const List = ({ items }) => <ul>{items.map((i) => <li>{i}</li>)}</ul>; // expect: jsx-key
