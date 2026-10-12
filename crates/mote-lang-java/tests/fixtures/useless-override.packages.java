package org.acme.web;

import org.acme.base.Connection;
import org.acme.web.support.*;

class Imported extends Connection {
    @Override
    protected void handleError(Throwable t) {
        super.handleError(t);
    }
}

class Wildcard extends Handler {
    @Override
    protected void handle() {
        super.handle();
    }
}

class Loader extends ClassLoader {
    @Override
    protected Class<?> findClass(String name) throws ClassNotFoundException {
        return super.findClass(name);
    }
}
