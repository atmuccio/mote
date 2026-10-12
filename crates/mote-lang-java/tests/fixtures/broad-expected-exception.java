// mote-test: test-file
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.fail;

import org.junit.Rule;
import org.junit.Test;
import org.junit.rules.ExpectedException;

public class EngineTest {
    @Rule
    public ExpectedException thrown = ExpectedException.none();

    private final Config config = new Config();

    @Test(expected = IllegalStateException.class)                  // expect: broad-expected-exception
    public void rejectsSecondStart() {
        Engine engine = new Engine(config);
        engine.start();
        engine.start();
    }

    @Test(expected = IllegalArgumentException.class, timeout = 1000) // expect: broad-expected-exception
    public void rejectsNegative() {
        Engine engine = new Engine(config);
        engine.setSpeed(-1);
    }

    @Test
    public void rejectsNull() {
        thrown.expect(NullPointerException.class);
        thrown.expectMessage("config");                             // expect: broad-expected-exception
        Engine engine = new Engine(null);
        engine.start();
        assertEquals(0, engine.speed());
    }

    @Test(expected = IllegalStateException.class)
    public void single() {
        new Engine(config).start();
    }

    @Test(expected = IllegalStateException.class)
    public void singleWithFail() {
        int speed = 3;
        engine().setSpeed(speed);
        fail("should have thrown");
    }

    @Test(expected = IllegalStateException.class)
    public void lambdasDoNotRun() {
        Runnable r = () -> engine().start();
        run(r);
    }

    @Test
    public void narrow() {
        Engine engine = new Engine(config);
        thrown.expect(IllegalStateException.class);
        engine.start();
    }

    // A no-argument construction is not setup that could throw.
    @Test(expected = IllegalStateException.class)
    public void trivialConstruction() {
        Engine engine = new Engine();
        engine.start();
    }

    @Test(expected = IllegalStateException.class)                    // expect: broad-expected-exception
    public void constructionWithArguments() {
        Engine engine = new Engine(new Config());
        engine.start();
    }

    @Test
    public void notAnExpectation() {
        Recorder recorder = new Recorder();
        recorder.expect("a");
        recorder.play();
        recorder.verify();
    }

    @Test
    public void noExpectation() {
        engine().start();
        engine().stop();
    }

    Engine engine() { return new Engine(config); }
    void run(Runnable r) { r.run(); }
}
