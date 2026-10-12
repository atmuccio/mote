import React from 'react';

export function Form({ save, name }) {
  return (
    <form>
      <label class="field" for="name">Name</label> {/* expect: no-unknown-property, no-unknown-property */}
      <input id="name" tabindex="1" maxlength={20} defaultValue={name} /> {/* expect: no-unknown-property, no-unknown-property */}
      <button onclick={save}>Save</button> {/* expect: no-unknown-property */}
      <div onmouseenter={save} ondblclick={save} /> {/* expect: no-unknown-property, no-unknown-property */}
      <svg viewbox="0 0 10 10"> {/* expect: no-unknown-property */}
        <path stroke-width="2" d="M0 0" /> {/* expect: no-unknown-property */}
      </svg>
    </form>
  );
}

// ---- compliant ----
export function Good({ save, props }) {
  return (
    <form>
      <label className="field" htmlFor="name">Name</label>
      <input id="name" tabIndex={1} maxLength={20} data-testid="x" aria-label="name" />
      <button onClick={save} type="submit">Save</button>
      <Button class="primary" onclick={save} />
      <my-widget class="custom" onclick={save}></my-widget>
      <button is="fancy-button" class="x" />
      <svg viewBox="0 0 10 10">
        <path strokeWidth={2} d="M0 0" />
      </svg>
      <div {...props} />
      <div onCustomthing={save} />
    </form>
  );
}

function Button(props) {
  return <button {...props} />;
}
