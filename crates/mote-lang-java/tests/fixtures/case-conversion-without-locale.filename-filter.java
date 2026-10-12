import java.awt.FileDialog;
import java.awt.Frame;
import java.io.File;

class Picker {
    // A FilenameFilter's second parameter is the file name.
    void pick(Frame frame, File dir) {
        FileDialog dialog = new FileDialog(frame, "Open");
        dialog.setFilenameFilter((d, name) -> name.toLowerCase().endsWith(".pdf"));   // expect: case-conversion-without-locale
        String[] pdfs = dir.list((d, name) -> name.toUpperCase().endsWith(".PDF"));   // expect: case-conversion-without-locale
        File[] files = dir.listFiles((File d, String n) -> n.toLowerCase().endsWith(".txt")); // expect: case-conversion-without-locale
    }

    void notFileNames(File dir, Chooser chooser, Grid grid) {
        File[] dirs = dir.listFiles(f -> f.getName().startsWith("x"));   // a FileFilter: the parameter is a File
        chooser.setFilenameFilter((d, name) -> name.toLowerCase().isEmpty()); // unknown library type
        grid.forEachCell((row, label) -> label.toLowerCase());          // unknown method
        dir.list((d, name) -> d.toString().isEmpty());
    }
}
