function check(config, key, data) {
  if (config.hasOwnProperty('port')) {} // expect: no-prototype-builtins
  if (config.hasOwnProperty(key)) {} // expect: no-prototype-builtins
  const own = data['hasOwnProperty']('id'); // expect: no-prototype-builtins
  const proto = Base.isPrototypeOf(config); // expect: no-prototype-builtins
  const enumerable = config.propertyIsEnumerable(key); // expect: no-prototype-builtins
  const maybe = data?.hasOwnProperty(key); // expect: no-prototype-builtins
  return this.hasOwnProperty(key) // expect: no-prototype-builtins
    && own && proto && enumerable && maybe;
}

function nested(obj) {
  return obj.inner.settings.hasOwnProperty('x'); // expect: no-prototype-builtins
}

// Compliant forms.
function ok(config, key) {
  const a = Object.hasOwn(config, key);
  const b = Object.prototype.hasOwnProperty.call(config, key);
  const c = Object.prototype.propertyIsEnumerable.call(config, key);
  const d = {}.hasOwnProperty.call(config, key);
  const e = Reflect.has(config, key);
  const f = Base.prototype.isPrototypeOf(config); // checking a class's prototype chain
  const g = Object.prototype.isPrototypeOf(config);
  const hasOwnProperty = Object.prototype.hasOwnProperty;
  const h = hasOwnProperty.call(config, key);
  const i = config.hasOwn(key); // some other method
  const j = config[key](1);
  return a && b && c && d && e && f && g && h && i && j;
}

class Model extends Base {
  has(key) {
    return super.hasOwnProperty(key);
  }
}

const bound = Object.prototype.hasOwnProperty.bind(target);
