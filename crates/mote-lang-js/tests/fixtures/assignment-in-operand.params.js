// mote-test: params report_array_index=true
buf[pos++] = 0; // expect: assignment-in-operand
const c = buf[--pos]; // expect: assignment-in-operand
