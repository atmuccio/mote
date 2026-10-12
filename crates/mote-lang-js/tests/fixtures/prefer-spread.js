function demo(values, parts, args, logger, fn) {
  Math.max.apply(Math, values); // expect: prefer-spread
  logger.warn.apply(logger, parts); // expect: prefer-spread
  fn.apply(undefined, args); // expect: prefer-spread
  fn.apply(null, args); // expect: prefer-spread
  fn.apply(void 0, args); // expect: prefer-spread
  this.items.push.apply(this.items, values); // expect: prefer-spread
  return this.run.apply(this, args); // expect: prefer-spread
}

// Compliant.
function ok(values, args, handler, context, fn, logger) {
  Math.max(...values);
  handler.apply(context, args); // a different `this` is the point
  fn.apply(this, args); // `this` is not what a plain call would pass
  Math.max.apply(null, values); // `null` instead of `Math`
  logger.warn.apply(other, args);
  fn.apply(null, [1, 2]); // a literal list: not a spread case
  fn.apply(null);
  fn.apply(null, args, extra);
  fn.apply(null, ...rest);
  getLogger().warn.apply(getLogger(), args); // the receiver is evaluated twice
  list[index].apply(list[index], args);
  fn?.apply(null, args);
  fn.call(null, ...args);
  Reflect.apply(fn, null, args);
  return fn.bind(null, args);
}

export { demo, ok };
