import java.util.logging.Level;
import java.util.logging.Logger;

class Loader {
    private static final Logger LOGGER = Logger.getLogger(Loader.class.getName());

    Object load(String name) throws ClassNotFoundException {
        try {
            return Class.forName(name);
        } catch (ClassNotFoundException e) {
            LOGGER.log(Level.SEVERE, "cannot load " + name, e);             // expect: log-and-rethrow
            throw e;
        }
    }

    Object loadQuietly(String name) throws ClassNotFoundException {
        try {
            return Class.forName(name);
        } catch (ClassNotFoundException e) {
            LOGGER.log(Level.FINE, "cannot load " + name, e);
            throw e;
        }
    }

    Object loadWarn(String name) throws ClassNotFoundException {
        try {
            return Class.forName(name);
        } catch (ClassNotFoundException e) {
            LOGGER.warning("cannot load " + name + ": " + e);               // expect: log-and-rethrow
            throw e;
        }
    }
}
