const label = n === 0 ? 'none' : n === 1 ? 'one' : 'many'; // expect: no-nested-ternary
const chained = a ? 1 : b ? 2 : c ? 3 : 4; // expect: no-nested-ternary
const inTest = (a ? b : c) ? 1 : 2; // expect: no-nested-ternary
const inConsequent = a ? (b ? 1 : 2) : 3; // expect: no-nested-ternary
const both = a
  ? b ? 1 : 2 // expect: no-nested-ternary
  : c ? 3 : 4;
const flat = a ? 1 : 2;
const inCall = a ? f(b ? 1 : 2) : 3;
const inArray = a ? [b ? 1 : 2] : [];
const inArrow = a ? () => (b ? 1 : 2) : null;
const inTemplate = a ? `${b ? 1 : 2}` : '';
const sequential = (a ? 1 : 2) + (b ? 3 : 4);
function pick() {
  return a ? 1 : 2;
}
