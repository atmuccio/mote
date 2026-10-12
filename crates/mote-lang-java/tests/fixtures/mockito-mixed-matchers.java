// mote-test: test-file
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyLong;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

import org.mockito.ArgumentMatchers;
import org.mockito.Mockito;

class RepoTest {
    Repo repo;
    Mailer mailer;
    static final String SUBJECT = "hi";

    void positives(String to, User user) {
        when(repo.find(anyLong(), "active")).thenReturn(user);             // expect: mockito-mixed-matchers
        verify(mailer).send(eq(to), SUBJECT);                               // expect: mockito-mixed-matchers
        verify(mailer).send(ArgumentMatchers.anyString(), to);              // expect: mockito-mixed-matchers
        Mockito.doReturn(user).when(repo).find(any(), new Object());         // expect: mockito-mixed-matchers
    }

    void negatives(String to, User user) {
        when(repo.find(anyLong(), eq("active"))).thenReturn(user);
        when(repo.find(1L, "active")).thenReturn(user);
        verify(mailer).send(eq(to), eq(SUBJECT));
        verify(mailer).send(eq(to), subjectMatcher());
        verify(mailer).send(eq(to));
        when(repo.find(anyLong(), status())).thenReturn(user);
    }

    String subjectMatcher() {
        return ArgumentMatchers.startsWith("h");
    }

    String status() {
        return "x";
    }

    interface Repo {
        User find(Object id, Object status);
    }

    interface Mailer {
        void send(String to, String subject);
        void send(String to);
    }

    static class User {}
}

class MatcherInLocalTest {
    RepoTest.Repo repo;

    void matcherHeldInLocal() {
        String status = ArgumentMatchers.anyString();
        Object id = ArgumentMatchers.any();
        when(repo.find(id, status)).thenReturn(null);
        when(repo.find(anyLong(), status)).thenReturn(null);
    }
}
