import java.io.InputStream;
import java.io.Reader;
import java.security.cert.X509Certificate;
import java.sql.Connection;
import java.sql.NClob;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.sql.SQLType;
import java.sql.Savepoint;
import java.sql.Statement;
import java.util.Map;
import java.util.concurrent.Executor;
import javax.net.ssl.X509TrustManager;
import javax.sql.DataSource;
import javax.tools.SimpleJavaFileObject;
import javax.xml.transform.ErrorListener;
import javax.xml.transform.Source;
import javax.xml.transform.TransformerException;
import javax.xml.transform.URIResolver;
import org.w3c.dom.*;
import com.acme.Pool;

abstract class TrackingConnection implements Connection {
    private final Connection target;

    TrackingConnection(Connection target) { this.target = target; }

    public int getNetworkTimeout() throws SQLException { return target.getNetworkTimeout(); } // expect: missing-override
    public void setNetworkTimeout(Executor e, int ms) throws SQLException { }                   // expect: missing-override
    public PreparedStatement prepareStatement(String sql, int[] keys) throws SQLException {     // expect: missing-override
        return target.prepareStatement(sql, keys);
    }
    public PreparedStatement prepareStatement(String sql, String[] names) throws SQLException { // expect: missing-override
        return target.prepareStatement(sql, names);
    }
    public Savepoint setSavepoint(String name) throws SQLException { return null; }            // expect: missing-override
    public void releaseSavepoint(Savepoint point) throws SQLException { }                      // expect: missing-override
    public Map<String, Class<?>> getTypeMap() throws SQLException { return null; }             // expect: missing-override
    public void beginRequest() throws SQLException { }                                         // expect: missing-override

    // Not in the interface: a different parameter type or arity.
    public PreparedStatement prepareStatement(String sql, long flags) { return null; }
    public void setNetworkTimeout(int ms) { }
    // Already annotated.
    @Override
    public void commit() throws SQLException { target.commit(); }
}

abstract class LoggingStatement implements PreparedStatement {
    public void setNString(int index, String value) throws SQLException { }                           // expect: missing-override
    public void setNClob(int index, NClob value) throws SQLException { }                              // expect: missing-override
    public void setNClob(int index, Reader reader, long length) throws SQLException { }               // expect: missing-override
    public void setBinaryStream(int index, InputStream in, long length) throws SQLException { }       // expect: missing-override
    public void setObject(int index, Object x, SQLType type) throws SQLException { }                  // expect: missing-override
    public int getResultSetType() throws SQLException { return ResultSet.TYPE_FORWARD_ONLY; }         // expect: missing-override
    public int executeUpdate(String sql, int[] columns) throws SQLException { return 0; }             // expect: missing-override
    public long executeLargeUpdate() throws SQLException { return 0L; }                               // expect: missing-override

    // Overloads that the JDBC interfaces do not declare.
    public void setNString(String name, String value, int flags) { }
    public void setNClob(int index, String value) { }
}

abstract class Source2 implements DataSource {
    public java.io.PrintWriter getLogWriter() { return null; }  // expect: missing-override
    public int getLoginTimeout() { return 0; }                 // expect: missing-override
}

abstract class Wrapped extends Pool implements Statement {
    // `Pool` is unknown: it may declare `cancel()` as well, which changes nothing here.
    public void cancel() throws SQLException { }               // expect: missing-override
    public void drain() { }
}

abstract class AdapterElement implements Element {
    public String getTagName() { return "item"; }                               // expect: missing-override
    public String getAttribute(String name) { return ""; }                      // expect: missing-override
    public boolean hasAttribute(String name) { return false; }                  // expect: missing-override
    public NodeList getElementsByTagName(String name) { return null; }          // expect: missing-override
    public Node appendChild(Node child) throws DOMException { return child; }   // expect: missing-override
    public short getNodeType() { return Node.ELEMENT_NODE; }                    // expect: missing-override
    public String getNodeName() { return getTagName(); }                        // expect: missing-override
    public Object getUserData(String key) { return null; }                      // expect: missing-override

    // Not DOM methods.
    public String getAttribute(String ns, String local, boolean strict) { return ""; }
    public void render() { }
}

abstract class TextNode implements Text {
    public String getData() { return ""; }                  // expect: missing-override
    public String getWholeText() { return getData(); }      // expect: missing-override
    public Text splitText(int offset) { return this; }      // expect: missing-override
}

class Children {
    NodeList of(final Node[] nodes) {
        return new NodeList() {
            public Node item(int i) { return nodes[i]; }    // expect: missing-override
            public int getLength() { return nodes.length; } // expect: missing-override
            public int size() { return nodes.length; }
        };
    }
}

class TrustAll implements X509TrustManager {
    public void checkClientTrusted(X509Certificate[] chain, String auth) { }  // expect: missing-override
    public void checkServerTrusted(X509Certificate[] chain, String auth) { }  // expect: missing-override
    public X509Certificate[] getAcceptedIssuers() { return new X509Certificate[0]; } // expect: missing-override
    public void checkServerTrusted(X509Certificate[] chain) { }
}

class Listener implements ErrorListener, URIResolver {
    public void warning(TransformerException e) { }                         // expect: missing-override
    public void error(TransformerException e) { }                           // expect: missing-override
    public void fatalError(TransformerException e) throws TransformerException { throw e; } // expect: missing-override
    public Source resolve(String href, String base) { return null; }        // expect: missing-override
    public void error(String message) { }
}

class PluginLoader extends ClassLoader {
    protected Class<?> findClass(String name) throws ClassNotFoundException {                      // expect: missing-override
        throw new ClassNotFoundException(name);
    }
    protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {     // expect: missing-override
        return super.loadClass(name, resolve);
    }
    protected Class<?> findClass(String name, int version) { return null; }
}

class InMemorySource extends SimpleJavaFileObject {
    InMemorySource() { super(null, null); }
    public CharSequence getCharContent(boolean ignoreErrors) { return ""; } // expect: missing-override
    public CharSequence getCharContent() { return ""; }
}
