let count = 0;
count = count++; // expect: useless-increment
count = count--; // expect: useless-increment
class Counter {
  tick() {
    this.value = this.value++; // expect: useless-increment
  }
}
// Compliant.
count++;
count = ++count;
let next = count++;
count = next++;
const arr = [0];
arr[0] = arr[1]++;
export { Counter, count, next };
