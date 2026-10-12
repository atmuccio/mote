// Errors created as a statement and then dropped.
function check(user) {
  if (!user) {
    new Error('user is required'); // expect: exception-not-thrown
  }
  if (user.age < 0) new RangeError('age must be positive'); // expect: exception-not-thrown
  new ValidationError(user); // expect: exception-not-thrown
  new errors.NotFoundError(user.id); // expect: exception-not-thrown
  new TypeError; // expect: exception-not-thrown
}

class Service {
  run() {
    if (!this.ready) {
      new Error(`not ready: ${this.name}`); // expect: exception-not-thrown
    }
  }
}

// Compliant: thrown, returned, stored, passed on, or not an error at all.
function ok(user) {
  if (!user) throw new Error('user is required');
  const err = new Error('kept');
  reject(new TypeError('passed'));
  callback(null, new ValidationError(user));
  new Widget(user);
  new ErrorReporter(user);
  new Map();
  void new Error('explicitly discarded');
  return new Error('returned');
}

const make = () => new Error('arrow body returns it');
const makeAll = (items) => items.map((i) => new ValidationError(i));
export { check, Service, ok, make, makeAll };
