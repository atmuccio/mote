import React, { useState } from 'react';

const Button = ({ on_click }: { on_click: () => void }) => <button onClick={on_click} />;
const StyledBox = styled.div`color: red;`;
const ThemeContext = React.createContext(null);
const LazyPage = React.lazy(() => import('./Page'));
export function UserCard({ user_name }: { user_name: string }) {
  const [is_open, setIsOpen] = useState(false);
  const Icon = is_open ? OpenIcon : ClosedIcon;
  const item_count = 3; // expect: naming-convention
  return <Icon label={user_name} count={item_count} />;
}
function render_row(row_data: unknown) { // expect: naming-convention, naming-convention
  return <div>{String(row_data)}</div>;
}
class Panel extends React.Component<Props_type> {
  UNSAFE_componentWillReceiveProps() {}
  handle_click = () => {};
  render_body() { return null; }
}
class plain_panel { // expect: naming-convention
  handle_click = () => {};
  render_body() { return null; } // expect: naming-convention
}
export default Button;
