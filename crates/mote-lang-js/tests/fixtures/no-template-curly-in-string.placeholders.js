const actorId = '12345';
const user = { name: 'Ada' };

// Test titles document route templates: labels, not messages.
describe('GET /actors/${actorId}', () => {
  it('returns ${actorId} details', () => {});
  it.only('GET /actors/${actorId}/roles', () => {});
});

export function config() {
  // A variable holding an object: `${parent}` is the product's own placeholder syntax.
  const parent = new Field('parent');
  const settings = { id: 1 };
  return {
    restUrl: 'example.com/cities?country=${parent}',
    other: 'example.com/items?id=${settings}',
  };
}

// Still reported: a string built for a request or a message.
export function request() {
  fetch('/api/actors/${actorId}'); // expect: no-template-curly-in-string
  return 'Hello ${user.name}!'; // expect: no-template-curly-in-string
}

function Field(name) {
  this.name = name;
}
