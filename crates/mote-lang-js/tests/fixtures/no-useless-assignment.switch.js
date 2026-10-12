// A switch with a default clause in which every clause overwrites the variable.
export function chartKind(type) {
  let kind = ''; // expect: no-useless-assignment
  switch (type) {
    case 'pieChart':
      kind = 'pie';
      break;
    case 'lineChart':
    case 'areaChart':
      kind = 'line';
      break;
    case 'none':
      return null;
    default:
      kind = 'bar';
  }
  return kind;
}

export function position(where, cx) {
  let x = cx; // expect: no-useless-assignment
  switch (where) {
    case 'start':
      prepare();
    // falls through
    case 'middle':
      x = cx / 2;
      break;
    default:
      throw new Error(where);
  }
  return x;
}

// Compliant.
export function noDefault(type) {
  let kind = 'bar';
  switch (type) {
    case 'pieChart':
      kind = 'pie';
      break;
  }
  return kind;
}

export function oneClauseKeeps(type) {
  let kind = 'bar';
  switch (type) {
    case 'pieChart':
      kind = 'pie';
      break;
    case 'keep':
      break;
    default:
      kind = 'line';
  }
  return kind;
}

export function readFirst(type) {
  let kind = 'bar';
  switch (type) {
    case 'pieChart':
      kind = kind + 'pie';
      break;
    default:
      kind = 'line';
  }
  return kind;
}

export function testedValue(kind) {
  let label = kind;
  switch (label) {
    case 'a':
      label = 'A';
      break;
    default:
      label = 'other';
  }
  return label;
}

export function nestedBreak(type, flag) {
  let kind = 'bar';
  switch (type) {
    case 'pieChart':
      if (flag) break;
      kind = 'pie';
      break;
    default:
      kind = 'line';
  }
  return kind;
}

export function lastClauseFallsOff(type) {
  let kind = 'bar';
  switch (type) {
    default:
      kind = 'line';
      break;
    case 'keep':
  }
  return kind;
}
