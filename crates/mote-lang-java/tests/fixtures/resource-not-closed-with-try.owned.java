import java.io.BufferedInputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStreamWriter;
import java.util.Properties;
import java.util.jar.JarOutputStream;
import java.util.zip.ZipOutputStream;
import javax.imageio.stream.FileImageOutputStream;

class Builders {
    Properties load(String name) throws IOException {
        Properties props = new Properties();
        InputStream in = Builders.class.getResourceAsStream(name);         // expect: resource-not-closed-with-try
        props.load(in);
        in.close();
        return props;
    }

    void zip(File target, File source) throws IOException {
        ZipOutputStream zip = new ZipOutputStream(new FileOutputStream(target)); // expect: resource-not-closed-with-try
        copy(source, zip);
        zip.close();
    }

    void jar(File target) throws IOException {
        JarOutputStream jar = new JarOutputStream(new FileOutputStream(target)); // expect: resource-not-closed-with-try
        jar.flush();
        jar.close();
    }

    void write(File f, String text) throws IOException {
        OutputStreamWriter w = new OutputStreamWriter(new FileOutputStream(f), "UTF-8"); // expect: resource-not-closed-with-try
        w.write(text);
        w.close();
    }

    void image(File f, byte[] data) throws IOException {
        FileImageOutputStream out;                                          // expect: resource-not-closed-with-try
        out = new FileImageOutputStream(f);
        out.write(data);
        out.close();
    }

    void read(File f) throws IOException {
        BufferedInputStream in = new BufferedInputStream(new FileInputStream(f)); // expect: resource-not-closed-with-try
        int b = in.read();
        in.close();
    }

    // Not reported.
    void handedOver(File f) throws IOException {
        InputStream in = new FileInputStream(f);
        register(in);                                                       // never closed here: ownership may move
    }

    void closedBefore(File f) throws IOException {
        InputStream in = new FileInputStream(f);
        in.close();
        register(in);
    }

    InputStream opened(String name) {
        InputStream in = getClass().getResourceAsStream(name);
        return in;                                                          // the caller closes it
    }

    void managed(String name, Properties props) throws IOException {
        try (InputStream in = getClass().getResourceAsStream(name)) {
            props.load(in);
        }
    }

    void inMemory(byte[] data) throws IOException {
        InputStream in = new java.io.ByteArrayInputStream(data);
        consume(in);
        in.close();
    }

    // `getClass()` is inherited from Object: its result is a Class all the same.
    void loadsThroughGetClass(String name, Properties props) throws IOException {
        InputStream in = getClass().getResourceAsStream(name);              // expect: resource-not-closed-with-try
        props.load(in);
        in.close();
        InputStream other = this.getClass().getClassLoader().getResourceAsStream(name); // expect: resource-not-closed-with-try
        props.load(other);
        other.close();
        InputStream third = getClass().getResourceAsStream(name);
        props.load(third);                                                  // never closed here: ownership may move
        InputStream fourth = helper().getClass().getResourceAsStream(name); // expect: resource-not-closed-with-try
        fourth.read();
        fourth.close();
    }

    Object helper() { return this; }

    void unknownOpener(Loader loader) throws IOException {
        InputStream in = loader.getResourceAsStream("x");                   // not a Class or ClassLoader
        consume(in);
        in.close();
    }

    void copy(File f, ZipOutputStream zip) { }
    void register(InputStream in) { }
    void consume(InputStream in) { }
}

class Probes {
    boolean present(String name) {
        java.io.InputStream in = null;
        try {
            in = Probes.class.getResourceAsStream(name);
            if (in != null) {
                in.close();
            }
        } catch (Exception e) {
        }
        return in != null;
    }

    void opensOrCloses(java.io.File f, Reader reader) throws java.io.IOException {
        java.io.InputStream in = new java.io.FileInputStream(f);
        try {
            reader.open(in);                                                // kept by the reader
        } catch (java.io.IOException e) {
            in.close();                                                     // closed only on failure
            throw e;
        }
    }
}
