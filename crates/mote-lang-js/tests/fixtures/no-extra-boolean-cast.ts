if (!!(user as object)) {} // expect: no-extra-boolean-cast
if ((!!user) as boolean) {} // expect: no-extra-boolean-cast
const b: boolean = !!user;
