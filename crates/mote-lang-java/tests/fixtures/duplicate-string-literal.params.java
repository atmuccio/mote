// mote-test: params min_length=3 threshold=2
class Params {
    String a = "abc";                                        // expect: duplicate-string-literal
    String b = "abc";
    String c = "ab";
    String d = "ab";
}
