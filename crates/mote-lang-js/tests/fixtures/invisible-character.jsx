export function Price({ amount }) {
  return (
    <p title="10 EUR" lang="fr">
      <img alt=" x" title="y " />
      {amount} € ; total : {amount}
      <span>long​word</span> {/* expect: invisible-character */}
      <span title="a​b" /> {/* expect: invisible-character */}
      <b>{' €'}</b> {/* expect: invisible-character */}
      <i>Donau­dampf</i>
      <em>abc‮</em> {/* expect: invisible-character */}
    </p>
  );
}
