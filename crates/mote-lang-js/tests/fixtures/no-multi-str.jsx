const A = () => <div title="first \
second" />;
// expect-next-line: no-multi-str
const B = () => <div>{'inner \
text'}</div>;
