for (let i = 0; i < 10; i--) {} // expect: for-direction
for (let i = 10; i >= 0; i++) {} // expect: for-direction
for (let i = 0; i <= n; i -= 1) {} // expect: for-direction
for (let i = n; i > 0; i += 2) {} // expect: for-direction
for (let i = 0; 10 > i; i--) {} // expect: for-direction
for (let i = 0; i < 10; i += -1) {} // expect: for-direction
for (let i = 0; i < 10; i++) {}
for (let i = 10; i > 0; i--) {}
for (let i = 0; i < 10; i -= -1) {}
for (let i = 0; i < 10; i += step) {}
for (let i = 0; i < 10; j--) {}
for (let i = 0; 10 < i; i--) {}
for (let i = 0; i !== 10; i--) {}
for (let i = 0; i < 10; i++, j--) {}
for (;;) {}
for (let i = 0; i < 10;) {}
for (let i = 0, j = 10; i < j; i++, j--) {}
