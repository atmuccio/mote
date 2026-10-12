// mote-test: params max=8
const date = /^(\d{4})-(\d{2})-(\d{2})$/;
const email = /^[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}$/i;
const tokens = /^((\d+)|([a-z]+)|(\s*,\s*))+$/; // expect: regex-too-complex
const words = /(?:foo|bar|baz|qux|quux|corge|grault|garply|waldo|fred|plugh|xyzzy)/;
const nested = /^((a|b|c)+(d|e)*)+(?=x|y)$/;
const long = /abcdefghijklmnopqrstuvwxyz0123456789/;
const constructed = new RegExp('((\\d+)|([a-z]+)|(\\s*,\\s*))+');
const unicode = /\p{L}+(?:[\p{N}_-]+\p{L}*)*/u;
