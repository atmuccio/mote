import * as React from 'react';
import { createContext, useContext, useEffect, useMemo, useReducer, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { create } from 'zustand';

interface Todo { id: string; title: string; done: boolean }
type Action = { type: 'toggle'; id: string } | { type: 'add'; todo: Todo };

const TodoContext = createContext<Todo[]>([]);

function reducer(state: Todo[], action: Action): Todo[] {
  switch (action.type) {
    case 'toggle':
      return state.map((t) => (t.id === action.id ? { ...t, done: !t.done } : t));
    case 'add':
      return [...state, action.todo];
    default:
      return state;
  }
}

export function useTodos(filter?: string) {
  const todos = useContext(TodoContext);
  const [query, setQuery] = useState(filter ?? '');
  const visible = useMemo(
    () => (query ? todos.filter((t) => t.title.includes(query)) : todos),
    [todos, query],
  );
  useEffect(() => {
    if (!filter) {
      return;
    }
    setQuery(filter);
  }, [filter]);
  return { visible, setQuery };
}

export const useBearStore = create<{ bears: number; inc: () => void }>((set) => ({
  bears: 0,
  inc: () => set((s) => ({ bears: s.bears + 1 })),
}));

export default function TodoPage({ userId }: { userId?: string }) {
  const [state, dispatch] = useReducer(reducer, []);
  const inputRef = useRef<HTMLInputElement>(null);
  const { data, isLoading } = useQuery({ queryKey: ['todos', userId], queryFn: () => fetchTodos(userId) });
  const { visible } = useTodos();
  const bears = useBearStore((s) => s.bears);

  const onSubmit = React.useCallback((e: React.FormEvent) => {
    e.preventDefault();
    const title = inputRef.current?.value;
    if (!title) {
      return;
    }
    dispatch({ type: 'add', todo: { id: String(Date.now()), title, done: false } });
  }, []);

  if (isLoading) {
    return <p>Loading…</p>;
  }

  return (
    <TodoContext.Provider value={state}>
      <form onSubmit={onSubmit}>
        <input ref={inputRef} />
      </form>
      <ul>
        {(data ?? visible).map((todo) => (
          <li key={todo.id} onClick={() => dispatch({ type: 'toggle', id: todo.id })}>
            {todo.title}
          </li>
        ))}
      </ul>
      <p>{bears}</p>
    </TodoContext.Provider>
  );
}

export const Modal = React.forwardRef<HTMLDivElement, { open: boolean }>((props, ref) => {
  const [visible, setVisible] = useState(props.open);
  useEffect(() => setVisible(props.open), [props.open]);
  if (!visible) {
    return null;
  }
  return <div ref={ref} />;
});

export const LazyPage = React.lazy(() => import('./page'));

export function Broken({ items, show }: { items: Todo[]; show: boolean }) {
  const [count, setCount] = useState(0);
  const total = show ? useMemo(() => items.length, [items]) : 0; // expect: rules-of-hooks
  items.forEach((item) => {
    useEffect(() => console.log(item), [item]); // expect: rules-of-hooks
  });
  return <span onClick={() => setCount(count + 1)}>{total}</span>;
}

declare function fetchTodos(id?: string): Promise<Todo[]>;
