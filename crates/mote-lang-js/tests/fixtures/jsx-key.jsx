import React, { Children, Fragment } from 'react';

export function UserList({ users, rows, groups, children }) {
  const header = [<th>Name</th>, <th>Email</th>]; // expect: jsx-key, jsx-key
  return (
    <div>
      <table>
        <thead><tr>{header}</tr></thead>
        <tbody>
          {users.map((user) => <tr><td>{user.name}</td></tr>)} {/* expect: jsx-key */}
          {rows.map(function (row) {
            if (row.hidden) {
              return null;
            }
            return <Row data={row} />; // expect: jsx-key
          })}
        </tbody>
      </table>
      {groups.flatMap((g) => g.items.map((item) => (
        <Item // expect: jsx-key
          value={item}
        />
      )))}
      {Array.from(rows, (row) => <Cell row={row} />)} {/* expect: jsx-key */}
      {users.map((u) => u.admin ? <Admin user={u} /> : <Member key={u.id} user={u} />)} {/* expect: jsx-key */}
      {users.map((u) => u.visible && <li>{u.name}</li>)} {/* expect: jsx-key */}
      {users.map((u) => <>{u.name}</>)} {/* expect: jsx-key */}

      {/* Compliant */}
      {users.map((user) => <li key={user.id}>{user.name}</li>)}
      {users.map((user) => <Fragment key={user.id}>{user.name}</Fragment>)}
      {users.map((user) => <UserRow {...user} />)}
      {users.map((user) => renderUser(user))}
      {users.map(renderUser)}
      {Children.map(children, (child) => <div className="wrap">{child}</div>)}
      {React.Children.toArray([<A />, <B />])}
      {Children.toArray(users.map((u) => <li>{u.name}</li>))}
      {users.map((user) => (
        <li key={user.id}>
          <span>{user.name}</span>
          {user.tags.map((tag) => <Tag key={tag} label={tag} />)}
        </li>
      ))}
      {users.map((user) => {
        const onClick = () => <Tooltip />;
        return <li key={user.id} onClick={onClick} />;
      })}
    </div>
  );
}

export function useDialog() {
  const [open, setOpen] = React.useState(false);
  return [open, <Dialog open={open} onClose={() => setOpen(false)} />];
}

const names = ['a', 'b'].map((n) => n.toUpperCase());
const empty = [];
const mixed = [1, <A />];
