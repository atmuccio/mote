export const Form = ({ save, props }) => (
  <form>
    <input type="text" value="a" type="number" /> {/* expect: jsx-no-duplicate-props */}
    <Button onClick={save} onClick={props.onClick} /> {/* expect: jsx-no-duplicate-props */}
    <svg><use xlink:href="#a" xlink:href="#b" /></svg> {/* expect: jsx-no-duplicate-props */}
    <Button
      label="Save"
      disabled
      label="Cancel" // expect: jsx-no-duplicate-props
    />

    {/* Compliant */}
    <input type="text" value="a" />
    <Button {...props} onClick={save} />
    <Button onClick={save} onclick={save} />
    <Button onClick={save}><Button onClick={save} /></Button>
    <input value="a" {...props} value="b" /> {/* expect: jsx-no-duplicate-props */}
  </form>
);
