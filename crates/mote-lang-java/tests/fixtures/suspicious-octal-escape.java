class Codes {
    String a = "\128";                                              // expect: suspicious-octal-escape
    String b = "id\477";                                            // expect: suspicious-octal-escape
    String c = "\0019";                                             // expect: suspicious-octal-escape
    String d = "x\09y";                                             // expect: suspicious-octal-escape

    // Not reported.
    String ok1 = "\0";
    String ok2 = "a\12b";
    String ok3 = "\177";
    String ok4 = "\\12";                                            // escaped backslash
    String ok5 = "\n8";
    String ok6 = "\t1";
    String ok7 = "\u00801";
    char ok8 = '\12';
    String ok9 = "\12" + "8";
    String ok10 = "\"1";
}
