class Access {
    void grant(User user, Log log, int n) {
        if (user.isAdmin())
            grantAccess();
            log.record(user);                                       // expect: misleading-indentation

        for (int i = 0; i < n; i++)
            step(i);
            finish();                                               // expect: misleading-indentation

        if (user.isGuest()) {
            deny();
        } else
            allow();
            audit();                                                // expect: misleading-indentation

        if (a())
            b();
        else if (c())
            d();
            e();                                                    // expect: misleading-indentation

        while (more())
            next();
            done();                                                 // expect: misleading-indentation

        if (x())
            for (Item it : items())
                use(it);
            after();                                                // expect: misleading-indentation
    }

    void fine(User user, Log log) {
        if (user.isAdmin())
            grantAccess();
        log.record(user);

        if (user.isAdmin()) {
            grantAccess();
            log.record(user);
        }

        if (user.isGuest()) deny();
        allow();

        if (user.isGuest()) deny();
            allow();                                                // body on the keyword's line: not modelled

        for (Item it : items())
            use(it);

        if (a())
            b();
        else
            c();
        d();

        if (a())
            b(1,
              2);
        d();

        if (a())
          b();
          // a comment line is not a statement
        d();
    }
}
