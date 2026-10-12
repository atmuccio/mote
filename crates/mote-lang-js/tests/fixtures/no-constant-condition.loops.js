// mote-test: params check_loops=true
while (true) {} // expect: no-constant-condition
do {} while (0); // expect: no-constant-condition
for (; 'x'; ) {} // expect: no-constant-condition
for (;;) {}
while (running) {}
if (false) {} // expect: no-constant-condition
