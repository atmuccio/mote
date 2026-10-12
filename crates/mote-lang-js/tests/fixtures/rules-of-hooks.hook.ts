import * as React from 'react';
import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';

export function useWindowSize(enabled: boolean) {
  const [size, setSize] = useState(0);
  if (enabled) {
    useEffect(() => setSize(window.innerWidth), []); // expect: rules-of-hooks
  }
  while (size < 0) {
    useState(1); // expect: rules-of-hooks
  }
  return size;
}

export const useCounter = (start: number) => {
  const [n, setN] = useState(start);
  return [n, () => setN(n + 1)] as const;
};

type Reader = (key: string) => unknown;

// A custom hook handed out by another hook, memoized so its identity stays stable: its hooks run
// in the render of the component that calls it.
export function useStore(source: Record<string, unknown>) {
  const listeners = useRef<Array<() => void>>([]);

  const useEntry: Reader = useCallback((key: string) => {
    const subscribe = useCallback((listener: () => void) => {
      listeners.current.push(listener);
      return () => undefined;
    }, []);
    const read = useCallback(() => source[key], [key]);
    if (!key) {
      useState(0); // expect: rules-of-hooks
    }
    return useSyncExternalStore(subscribe, read, read);
  }, []);

  const useLegacyEntry = React.useCallback(function (key: string) {
    return useMemo(() => source[key], [key]);
  }, []) as Reader;

  // Not a hook: a plain callback, so hooks inside it are still reported.
  const watch = useCallback((key: string) => {
    useEffect(() => undefined, [key]); // expect: rules-of-hooks
  }, []);

  // Not the function itself: `useMemo` calls the factory during this render.
  const useDerived = useMemo(() => {
    useEffect(() => undefined, []); // expect: rules-of-hooks
    return () => source;
  }, []);

  return { useEntry, useLegacyEntry, watch, useDerived };
}
