import React, { useState, useEffect, useMemo, useCallback, memo, forwardRef } from 'react';

export function Profile({ user, items }) {
  if (user.admin) {
    useAudit(user); // expect: rules-of-hooks
  }
  const theme = user.dark ? useDarkTheme() : useLightTheme(); // expect: rules-of-hooks, rules-of-hooks
  const flag = user.beta && useFeature('beta'); // expect: rules-of-hooks
  for (const item of items) {
    useTrack(item); // expect: rules-of-hooks
  }
  switch (user.kind) {
    case 'a':
      useKindA(); // expect: rules-of-hooks
      break;
    default:
  }
  useEffect(() => {
    const value = useContext(Ctx); // expect: rules-of-hooks
  }, []);
  const onClick = useCallback(() => {
    useLogger().log('click'); // expect: rules-of-hooks
  }, []);
  function renderRow(row) {
    const t = useTranslation(); // expect: rules-of-hooks
    return <Row row={row} t={t} />;
  }
  if (!user) {
    return null;
  }
  const [tab, setTab] = useState('info'); // expect: rules-of-hooks
  return <div onClick={() => React.useRef()}>{theme}{flag}{tab}</div>; // expect: rules-of-hooks
}

export const Card = ({ loading }) => {
  const [open, setOpen] = useState(false);
  if (useIsMobile()) {
    setOpen(true);
  }
  const value = useMemo(() => compute(open), [open]);
  if (loading) {
    return <Spinner />;
  }
  return <div>{value}</div>;
};

export function useData(id) {
  const [data, setData] = useState(null);
  if (!id) return null;
  useEffect(() => { fetchData(id).then(setData); }, [id]); // expect: rules-of-hooks
  return data;
}

export function useSafe(id) {
  const [data, setData] = useState(null);
  useEffect(() => {
    if (!id) return;
    fetchData(id).then(setData);
  }, [id]);
  return data;
}

// Components through wrappers, nested components, and custom hooks defined inline.
export const Memo = memo(() => {
  const [x] = useState(0);
  return <div>{x}</div>;
});
export const Ref = forwardRef(function Inner(props, ref) {
  const v = useImperative(ref);
  return <input ref={ref} value={v} />;
});
export const Observed = observer(() => {
  const store = useStore();
  return <div>{store.name}</div>;
});
export function Outer() {
  const Inner = () => {
    const [y] = useState(1);
    return <span>{y}</span>;
  };
  function useLocal() {
    return useState(2);
  }
  return <Inner />;
}

// Not components or hooks: stay silent.
function helper() {
  if (x) {
    useSomething();
  }
}
class Legacy extends React.Component {
  render() {
    const v = useSomething();
    return <div>{v}</div>;
  }
}
describe('hooks', () => {
  it('works', () => {
    renderHook(() => useData(1));
  });
});
const story = {
  render: () => {
    const [v] = useState(0);
    return <div>{v}</div>;
  },
};
export function List({ items }) {
  const user = useUser();
  const loaded = items.use(1);
  const also = use(ctx);
  const maybe = user && user.useless();
  return <ul>{items.map((i) => <li key={i}>{i}</li>)}</ul>;
}
