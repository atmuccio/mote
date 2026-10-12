interface Box { value?: number; next?: Box }
declare const box: Box;
declare const other: number;

if (box.value! == other) {} // expect: no-confusing-non-null-assertion
if (box.value! === other) {} // expect: no-confusing-non-null-assertion
box.value! = 3; // expect: no-confusing-non-null-assertion
box.next!.value! = 4; // expect: no-confusing-non-null-assertion

// Parenthesized, other operators, or no assertion: fine.
if ((box.value!) == other) {}
(box.value!) = 5;
if (box.value != other) {}
if (box.value !== other) {}
if (box.value! > other) {}
if (other == box.value!) {}
box.value! += 1;
const sum = box.value! + other;
const eq = box.next!.value === other;

// Optional chains: the assertion ends the chain.
declare const maybe: Box | undefined;
if (maybe?.value! == other) {} // expect: no-confusing-non-null-assertion
if (maybe?.next?.value! === other) {} // expect: no-confusing-non-null-assertion
if ((maybe?.value!) == other) {}
if ((maybe?.value)! == other) {} // expect: no-confusing-non-null-assertion
if (maybe?.value != other) {}
if (maybe?.next!.value == other) {}
