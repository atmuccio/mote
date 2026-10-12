import java.io.*;
import java.util.*;
import java.util.function.*;

class LocalNames {
    void params(int goodParam, int Bad_Param, String... Rest_Args) {   // expect: local-variable-naming, local-variable-naming
    }

    LocalNames(int X, int Y_Value) {}                                  // expect: local-variable-naming

    void locals(List<String> list, Object o) throws Exception {
        int itemCount = 0;
        int Item_Count = 0;                                            // expect: local-variable-naming
        final int MAX_RETRIES = 3;                                     // expect: local-variable-naming
        int i = 0, J = 1, _k2 = 2;                                     // expect: local-variable-naming
        var parsedHTTP2 = "x";
        for (String Name : list) {                                     // expect: local-variable-naming
        }
        for (int Idx = 0; Idx < 3; Idx++) {                            // expect: local-variable-naming
        }
        try (Reader In_Reader = new StringReader("")) {                // expect: local-variable-naming
        } catch (IOException | RuntimeException Ex_Caught) {           // expect: local-variable-naming
        }
        try {
            o.hashCode();
        } catch (Exception E) {                                        // single letters are fine
        }
        if (o instanceof String Str_Value) {                           // expect: local-variable-naming
        }
        if (o instanceof String s) {
        }
        Function<Integer, Integer> f = Arg_X -> Arg_X + 1;             // expect: local-variable-naming
        BiFunction<Integer, Integer, Integer> g = (a, Second_Arg) -> a; // expect: local-variable-naming
        BiFunction<Integer, Integer, Integer> h = (Integer First_Typed, Integer b) -> b; // expect: local-variable-naming
        Runnable r = () -> {
            int Inner_Local = 1;                                       // expect: local-variable-naming
        };
        var _ = list.size();
        switch (o) {
            case Integer Int_Value -> {                                // expect: local-variable-naming
            }
            case Point(int Px, int py) -> {                            // expect: local-variable-naming
            }
            default -> {
            }
        }
    }

    record Point(int x, int Y_Coord) {}                                // expect: local-variable-naming
}
