// expect-next-line: no-multi-str
type Msg = 'a \
b';
const ok: Msg = 'a b' as Msg;
