let total = 0;
let count = 10;
let flag = false;
total =+ item.price; // expect: reversed-compound-operator
count =- 1; // expect: reversed-compound-operator
flag =! ready; // expect: reversed-compound-operator

// Compliant.
total += item.price;
count -= 1;
count = -1;
count = - 1;
count =-1;
flag = !ready;
flag = ! ready;
flag = total !== count;
export { total, count, flag };
