import { some_external, Other_Thing } from './external';
import default_import from './default';
import * as name_space from './ns';
import fs_extra = require('fs-extra');
const legacy_module = require('legacy-module');
const legacy_member = require('legacy-module').some_member;

// Types: PascalCase.
class user_service {} // expect: naming-convention
class UserService {}
interface props_type { user_id: string; get_value(): number } // expect: naming-convention
interface Props { user_id: string; 'quoted-key': number }
type user_id = string; // expect: naming-convention
type UserId = string;
enum status_kind { Active } // expect: naming-convention
enum Status { Active, INACTIVE, pending, In_Progress }
enum Wire { 'in-progress' = 1, 'done_now' = 2 }
type Mapper<T, TValue, k_v> = (value: T) => TValue | k_v; // expect: naming-convention
type Keys<T> = { [K in keyof T]: T[K] };
class URL2 {}
class T {}

// Functions: camelCase or PascalCase.
function load_user(user_id: string) {} // expect: naming-convention, naming-convention
function loadUser(userId: string, _unused: number, Ctor: unknown) {}
function UserCard() { return null; }
function DO_IT() {} // expect: naming-convention
function overloaded(a: string): void;
function overloaded(a: number): void;
function overloaded(a: unknown) {}
function bad_overload(a: string): void; // expect: naming-convention
function bad_overload(a: unknown) {}
type Callback = (user_id: string) => void;
interface Api { call(user_id: string): void; new (some_arg: number): Api }
declare function declared_fn(some_arg: string): void;
declare const declared_const: number;
declare global {
  interface Window { my_global: string }
  const global_thing: string;
}
declare module 'some-module' {
  export function module_fn(a_b: string): void;
}

// Variables.
let user_name = 'x'; // expect: naming-convention
let userName = 'x';
const MAX_RETRIES = 3;
const DEFAULT_OPTIONS = { retries: 3 };
export const API_ROUTES = ['/a'];
const ApiClient = createClient();
const Pascal_Snake = createClient(); // expect: naming-convention
const AppVersion = '1.0'; // expect: naming-convention
let Counter = 0; // expect: naming-convention
let Holder = null;
const { user_id, nested: { deep_value } } = getData();
const [first_item, second_item] = getPair();
const __dirname = '.';
const __webpack_public_path__ = '/';
var _private_thing = 1; // expect: naming-convention
var _privateThing = 1;
var trailing_ = 1;
const $el = document.body;
const value$ = observable();
const _ = 1;
const $ = 2;

function scope() {
  const LOCAL_CONSTANT = 1;
  let LOCAL_LET = 1; // expect: naming-convention
  const local_snake = compute(); // expect: naming-convention
  const isDraft2020_12 = true;
  const LocalPascal = 'x'; // expect: naming-convention
  const localConstant = 1;
  for (const item_id of [1]) {} // expect: naming-convention
  for (const [key_a, value_b] of new Map()) {}
  try {} catch (some_error) {} // expect: naming-convention
  try {} catch ({ message_text }) {}
  const arrow = (arg_one: number, ...rest_args: number[]) => arg_one; // expect: naming-convention, naming-convention
  const destruct = ({ snake_prop }: { snake_prop: number }, [first_el]: number[]) => snake_prop;
  return [arrow, destruct, LOCAL_CONSTANT, localConstant, LOCAL_LET, local_snake, isDraft2020_12, LocalPascal];
}

namespace Space {
  export const NAMESPACE_CONST = 1;
  export let namespace_let = 2;
  export const EnumLike = 'enum-like';
  const internal_value = 3; // expect: naming-convention
  export function space_fn() { return internal_value; } // expect: naming-convention
}

// Class members.
class Widget {
  user_field = 1;
  'quoted-field' = 2;
  static MAX_SIZE = 3;
  #secret_field = 4;
  constructor(private readonly user_repo: unknown, public some_flag: boolean, plain_param: number) {} // expect: naming-convention
  get_value() { return 1; } // expect: naming-convention
  getValue() { return 1; }
  GetOther() { return 2; } // expect: naming-convention
  #private_method() {} // expect: naming-convention
  #privateMethod() {}
  get some_getter() { return 1; }
  set some_setter(value_x: number) {} // expect: naming-convention
  'quoted-method'() {}
  ['computed_method']() {}
  static static_method() {} // expect: naming-convention
  UNSAFE_componentWillMount() {}
  overloaded_method(a: string): void;
  overloaded_method(a: unknown) {} // expect: naming-convention
}

abstract class Base {
  abstract do_work(): void; // expect: naming-convention
}

class Child extends Base {
  do_work() {}
  override_free_method() {}
}

class Impl implements Props {
  user_id = '';
  'quoted-key' = 1;
  snake_method() {}
}

class Overrides extends Base {
  override do_work() {}
}

const obj = {
  snake_key: 1,
  snake_method() {},
  arrow_key: () => 1,
  fn_key: function fn_key() {},
};
obj.assigned_fn = function assigned_fn() {};
module.exports.exported_fn = function exported_fn() {};
const named_expr = function inner_name() {}; // expect: naming-convention, naming-convention
const ClassHolder = class inner_class {}; // expect: naming-convention

export default function default_fn() {} // expect: naming-convention
export { user_name as renamed_export };

// Exemptions for names that mirror data formats or other code.
function mirrors(body: { indices_boost: number; 'page_size': number }, cfg?: { retry_count: number }) {
  const indices_boost = body.indices_boost;
  const page_size = body['page_size'];
  const retry_count = cfg?.retry_count;
  const other_name = body.indices_boost; // expect: naming-convention
  const depends_on = { db: 1 };
  return [{ depends_on, indices_boost, page_size, retry_count }, other_name];
}
const rows = [1, 2].map((case_field_id, index) => ({ case_field_id, order: index }));
const editor_1 = rows;
const MESSAGE_2 = 'two';
function numbered(item_2: number, x__1: number) { return item_2 + x__1; } // expect: naming-convention
class Routes {
  @Get('a_b') express_style() {}
  @Input() some_input = 1;
  plain_method() {} // expect: naming-convention
}
class Legacy {
  /**
   * Old name, kept for compatibility.
   * @deprecated use request()
   */
  public AjaxRequest() { return 1; }
  /** Not deprecated. */
  OtherRequest() { return 2; } // expect: naming-convention
}
/** @deprecated use newName */
export const old_name = 1;
/** @deprecated */
export function old_fn() {}
