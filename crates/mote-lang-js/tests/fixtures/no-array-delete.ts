declare function load(): number[];
const queue: number[] = load();
delete queue[0]; // expect: no-array-delete

function drop(items: string[], tuple: [number, string], generic: Array<number>, index: number) {
  delete items[index]; // expect: no-array-delete
  delete tuple[1]; // expect: no-array-delete
  delete generic[index]; // expect: no-array-delete
}

const literal = [1, 2, 3];
delete literal[1]; // expect: no-array-delete
let built = new Array(10);
delete built[3]; // expect: no-array-delete
const from = Array.from({ length: 3 });
delete from[0]; // expect: no-array-delete

class Queue {
  private jobs: string[] = [];
  private pending = [];
  private map: Record<string, number> = {};
  run(i: number) {
    delete this.jobs[i]; // expect: no-array-delete
    delete this.pending[i]; // expect: no-array-delete
    delete this.map['x'];
    const inner = () => { delete this.jobs[0]; }; // expect: no-array-delete
    return function (this: any) { delete this.jobs[0]; };
  }
}

// Compliant: objects, maps, unknown types, reassigned variables, property deletes.
const cache: Record<string, number> = {};
delete cache['key'];
const dict: { [k: string]: number } = {};
delete dict.key;
let reassigned = [1, 2];
reassigned = somethingElse();
delete reassigned[0];
function unknownParam(value: any, maybe: number[] | undefined, key: string) {
  delete value[key];
  delete maybe![0];
}
const obj = { list: [1, 2] };
delete obj.list[0];
queue.splice(0, 1);
declare function somethingElse(): any;
