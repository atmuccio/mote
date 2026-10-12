// @ts-check
export const App = () => (
  <div>
    {/* expect-next-line: ban-ts-comment */}
    {/* @ts-ignore */}
    <Legacy prop={1} />
    {/* @ts-expect-error: Legacy does not declare `prop` */}
    <Legacy prop={2} />
  </div>
);
