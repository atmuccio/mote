// mote-test: test-file
import * as api from './api.js';
import { fetchUser } from './api.js';

// Monkey-patching a namespace in tests is accepted (the runner compiles to CommonJS).
api.fetchUser = () => Promise.resolve({ id: 1 });
fetchUser = null; // expect: no-import-assign
