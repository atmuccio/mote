// No React in this file: `use…` functions are ordinary functions.
export function Configure(app: any) {
  if (app.dev) {
    useDevTools(app);
  }
}
declare function useDevTools(app: any): void;
