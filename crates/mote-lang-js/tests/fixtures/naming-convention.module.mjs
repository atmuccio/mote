import fs from 'fs';
var module_counter = 0; // expect: naming-convention
export function read_file(file_name) { // expect: naming-convention, naming-convention
  module_counter++;
  return fs.readFileSync(file_name);
}
