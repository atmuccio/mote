function check(input, event, value, text) {
  if (!isNaN(input.value)) { // expect: prefer-number-isnan
    save(input.value);
  }
  if (isFinite(event.key)) { // expect: prefer-number-isnan
    digit(event.key);
  }
  const label = 'x' + value;
  if (isNaN(label)) {} // expect: prefer-number-isnan
  if (isNaN(value)) {} // expect: prefer-number-isnan
  if (isNaN(text.trim())) {} // expect: prefer-number-isnan

  // Certainly numbers: the global and Number versions agree.
  if (isNaN(parseInt(text, 10))) {}
  if (isNaN(parseFloat(text))) {}
  if (isNaN(Number(text))) {}
  if (isNaN(Number.parseFloat(text))) {}
  if (isNaN(value / 2)) {}
  if (isNaN(+text)) {}
  if (isNaN(-value)) {}
  if (isFinite(Math.max(value, 3))) {}
  if (isNaN(new Date(text).getTime())) {}
  if (isNaN(Date.parse(text))) {}
  if (isNaN(text.length)) {}
  const parsed = parseFloat(text);
  if (isNaN(parsed)) {}
  let changing = parseFloat(text);
  changing = text;
  if (isNaN(changing)) {} // expect: prefer-number-isnan
  if (isNaN(value ? 1 : 2)) {}

  // Dates convert to their time value.
  if (isNaN(new Date(text))) {}
  const when = new Date(text);
  if (isNaN(when)) {}
  if (value instanceof Date && !isNaN(value)) {}
  if (Object.prototype.toString.call(value) === '[object Date]' && !isNaN(value)) {}

  // Guarded by a type check.
  if (typeof value === 'number' && !isNaN(value)) {}
  if (typeof value === 'number' && value > 0 && isFinite(value)) {}
  if (util.isNumber(value) && (isNaN(value) || !isFinite(value))) {}
  if (typeof text === 'number' && isNaN(value)) {} // expect: prefer-number-isnan

  // Not the global functions.
  if (Number.isNaN(value)) {}
  if (lib.isNaN(value)) {}
  if (isNaN()) {}
  if (isNaN(...args)) {}
}

function shadowed(isNaN, value) {
  return isNaN(value);
}

function converted(val, show, tokens, widget, text) {
  val = +val;
  if (isNaN(val)) {}
  var parsed;
  if (show) {
    parsed = parseFloat(text);
  } else {
    parsed = parseInt(text, 10);
  }
  if (isNaN(parsed)) {}
  var hours = parseInt(tokens[0]),
    seconds = show ? parseInt(tokens[2]) : null;
  if (isNaN(hours) || (show && isNaN(seconds))) {}
  if (isFinite(seconds)) {} // expect: prefer-number-isnan
  var page = widget.parseLocalizedInt(text);
  if (isNaN(page)) {}
  var year = widget.drawYear + parseInt(text, 10);
  if (isNaN(year)) {}
  if (/^-?\d+$/.test(String(text)) && isFinite(text)) {}
  if (/^\d+$/.test(text) && !isNaN(text)) {}
  var padded = text;
  padded += ' ';
  if (isNaN(padded)) {} // expect: prefer-number-isnan
  var later = text;
  if (isNaN(later)) {} // expect: prefer-number-isnan
  later = parseInt(text, 10);
}
