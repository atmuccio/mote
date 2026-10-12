// mote-test: test-file
import { render, fireEvent } from '@testing-library/react';
import { parse } from '../src/parser';
import { Table as ImportedTable } from '../src/table';

class Table {
  parse(text) { this.text = text; }
  reset() { this.rows = []; }
}

class Checker {
  accepts(v) { assert.ok(v); }
}
import * as sinon from 'sinon';

describe('table', () => {
  let shared;
  let fromImport;
  beforeEach(() => {
    shared = new Table();
    fromImport = parse('y');
  });

  it('parses the header', () => { // expect: test-without-assertion
    const table = new Table();
    table.parse('id;name');
  });

  test('renders', () => { // expect: test-without-assertion
    render('<App />');
    fireEvent.click(button);
  });

  it.only('only computes', function () { // expect: test-without-assertion
    const n = 1 + 2;
    String(n);
  });

  it('is an empty placeholder', () => {});

  it('was disabled by commenting it out', () => {
    // expect(parse('x')).toBe(1);
  });

  it('uses an app from the mocha context', async function () {
    const app = this.app;
    await app.workbench.open();
  });

  it('uses a local helper that does not assert', () => { // expect: test-without-assertion
    setUp();
  });

  it('uses an object from a hook that does not assert', () => { // expect: test-without-assertion
    shared.reset();
  });

  it('spies only', () => { // expect: test-without-assertion
    const spy = sinon.spy();
    spy();
  });

  // Compliant.
  it('asserts', () => {
    expect(parse('x')).toEqual(1);
  });

  it('asserts in a callback', () => {
    return load().then((r) => {
      expect(r).toBeDefined();
    });
  });

  it('asserts through a helper', () => {
    checkTable(new Table());
  });

  it('asserts through a nested helper', () => {
    outer();
  });

  it('calls imported code', () => {
    parse('x');
  });

  it('uses an imported class', () => {
    const t = new ImportedTable();
    t.parse('x');
  });

  it('uses a local class that asserts', () => {
    const c = new Checker();
    c.accepts(1);
  });

  it('uses a helper made by a local factory', () => {
    const check = makeChecker();
    check(1);
  });

  it('uses the mocha context', async function () {
    await this.app.open();
  });

  it('uses an object assigned from imported code in a hook', () => {
    fromImport.run();
  });

  it('uses chai should', () => {
    const t = new Table();
    t.rows.should.have.length(0);
  });

  it('uses node assert', () => {
    assert.strictEqual(1, 1);
  });

  it('uses sinon.assert', () => {
    const spy = sinon.spy();
    sinon.assert.notCalled(spy);
  });

  it('queries throw when missing', () => {
    render('<App />');
    screen.getByText('Hello');
  });

  it('throws', () => {
    if (bad()) throw new Error('bad');
  });

  it('waits for done', (done) => {
    setTimeout(done, 10);
  });

  it('is pending', () => {
    pending('later');
  });

  it('skips at runtime', function () {
    this.skip();
  });

  it.skip('skipped', () => {});
  it.todo('todo');
  test.each([1, 2])('each %i', (n) => {});

  it('fails explicitly', () => {
    fail('not implemented');
  });

  it('uses supertest', () => {
    return request(app).get('/').expect(200);
  });

  it('is written elsewhere', runCase);

  it('uses a global helper', () => {
    customAssertion();
  });
});

function setUp() {
  const t = new Table();
  t.reset();
  const list = [];
  list.push(1);
}

function makeChecker() {
  return (v) => expect(v).toBe(1);
}

function checkTable(t) {
  expect(t.rows).toHaveLength(0);
}

function outer() {
  inner();
}

function inner() {
  expect(1).toBe(1);
}
