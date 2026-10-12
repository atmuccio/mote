export class StringUtils { // expect: no-extraneous-class
  static capitalize(s: string) { return s.toUpperCase(); }
  static readonly SEPARATOR = '-';
}
class Blocked { // expect: no-extraneous-class
  private constructor() {}
  static create() { return 1; }
}
class EmptyCtor { // expect: no-extraneous-class
  constructor() {}
  static x = 1;
  static { init(); }
}
class WithInstance {
  static count = 0;
  value = 1;
}
class WithCtor {
  constructor(public a: number) {}
  static make() { return new WithCtor(1); }
}
class Empty {}
abstract class AbstractBase {
  static helper() {}
}
class Derived extends Base {
  static x = 1;
}
@Injectable()
class Decorated {
  static x = 1;
}
class Implements implements Thing {
  static x = 1;
}
declare class Ambient {
  static x: number;
}
class Merged {
  static x = 1;
}
interface Merged { y: number }
class ExtendedHere {
  static base() {}
}
class Sub extends ExtendedHere {}
class Instantiated {
  static y = 1;
}
const inst = new Instantiated();
const Expr = class {
  static z = 1;
};
class WithGetter {
  static get value() { return 1; }
  get other() { return 2; }
}
class WithIndex {
  static [key: string]: number;
}
class WithDecoratedMember {
  @Get('/x')
  static handler() {}
}
class WithParamDecorator {
  static handler(@Param('id') id: string) {}
}
class PassedAsValue {
  static forRoot() { return { module: PassedAsValue }; }
}
class UsedAsType {
  static x = 1;
}
let typed: UsedAsType;
class NamespaceUse { // expect: no-extraneous-class
  static run() { return NamespaceUse.helper(); }
  static helper() { return 1; }
}
NamespaceUse.run();
export { NamespaceUse };
