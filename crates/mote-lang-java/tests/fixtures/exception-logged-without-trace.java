import java.io.IOException;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

class Publisher {
    private static final Logger log = LoggerFactory.getLogger(Publisher.class);

    // The markers sit after the handlers: a comment inside a handler exempts it.
    void positives(Channel channel, Event event) {
        try {
            channel.send(event);
        } catch (IOException e) { log.error("could not publish: " + e.getMessage()); } // expect: exception-logged-without-trace
        try {
            channel.send(event);
        } catch (IOException e) {
            log.warn("could not publish {}: {}", event.id(), e.toString()); event.markFailed(); } // expect: exception-logged-without-trace
        try {
            channel.send(event);
        } catch (RuntimeException e) {
            log.error("could not publish " + e); return; } // expect: exception-logged-without-trace
        try {
            channel.send(event);
        } catch (IOException e) {
            String reason = "none";
            log.error("failed {}", e.getLocalizedMessage()); } // expect: exception-logged-without-trace
        // Comments that explain nothing: tool markers, task notes, commented-out code.
        try {
            channel.send(event);
        } catch (IOException e) { // CHECKSTYLE:OFF
            log.warn("send failed: {}", e.getMessage()); } // expect: exception-logged-without-trace
        try {
            channel.send(event);
        } catch (IOException e) { //NOPMD
            log.warn("send failed: {}", e.getMessage()); } // expect: exception-logged-without-trace
        try {
            channel.send(event);
        } catch (IOException e) {
            // TODO: report a specific error
            log.warn("send failed: {}", e.getMessage()); } // expect: exception-logged-without-trace
        try {
            channel.send(event);
        } catch (IOException e) {
            // e.printStackTrace();
            log.warn("send failed: {}", e.getMessage()); } // expect: exception-logged-without-trace
    }

    Publisher(Channel channel) {
        try {
            channel.open();
        } catch (IOException e) {
            log.warn("channel unavailable: {}", e.getMessage()); } // expect: exception-logged-without-trace
    }

    void negatives(Channel channel, Event event) {
        // The exception itself is passed.
        try {
            channel.send(event);
        } catch (IOException e) {
            log.error("could not publish: " + e.getMessage(), e);
        }
        // Placeholder arguments with the exception last.
        try {
            channel.send(event);
        } catch (IOException e) {
            log.warn("could not publish {}: {}", event.id(), e.getMessage(), e);
        }
        // Info level.
        try {
            channel.send(event);
        } catch (IOException e) {
            log.info("could not publish: " + e.getMessage());
        }
        // Falls back to a value.
        try {
            channel.send(event);
        } catch (IOException e) {
            log.warn("could not publish: " + e.getMessage());
            this.pending = event;
        }
    }

    int fallbackAfterTry(Channel channel) {
        try {
            return channel.count();
        } catch (IOException e) {
            log.warn("count unavailable: " + e.getMessage());
        }
        return -1;
    }

    int fallbackInHandler(Channel channel) {
        try {
            return channel.count();
        } catch (IOException e) {
            log.warn("count unavailable: " + e.getMessage());
            return -1;
        }
    }

    void moreNegatives(Channel channel, Event event) throws IOException {
        // Rethrown.
        try {
            channel.send(event);
        } catch (IOException e) {
            log.error("could not publish: " + e.getMessage());
            throw e;
        }
        // Wrapped.
        try {
            channel.send(event);
        } catch (IOException e) {
            log.error("could not publish: " + e.getMessage());
            throw new IllegalStateException(e);
        }
        // Kept elsewhere.
        try {
            channel.send(event);
        } catch (IOException e) {
            log.error("could not publish: " + e.getMessage());
            failures.add(e);
        }
        // The trace is logged separately.
        try {
            channel.send(event);
        } catch (IOException e) {
            log.error("could not publish: " + e.getMessage());
            log.debug("details", e);
        }
        // Handed to a lambda.
        try {
            channel.send(event);
        } catch (IOException e) {
            log.error("could not publish: " + e.getMessage());
            executor.execute(() -> handle(e));
        }
        // Not a logger.
        try {
            channel.send(event);
        } catch (IOException e) {
            console.error("could not publish: " + e.getMessage());
        }
        // The cause is used.
        try {
            channel.send(event);
        } catch (IOException e) {
            log.error("could not publish: " + e.getCause().getMessage());
        }
        // An interruption has no interesting trace.
        try {
            Thread.sleep(100);
        } catch (InterruptedException e) {
            log.warn("interrupted: {}", e.getMessage());
        }
        // The handler documents the failure as expected and the fallback that follows.
        try {
            channel.send(event);
        } catch (IOException e) { // treated like an empty queue below
            log.error(e.toString());
        }
        try {
            channel.send(event);
        } catch (IllegalArgumentException e) {
            // Malformed input is common here: keep the original value
            log.warn("malformed event, kept as is: {}", e.getMessage());
        }
        // Inside a lambda.
        Runnable r = () -> {
            try {
                channel.send(event);
            } catch (IOException e) {
                log.error("could not publish: " + e.getMessage());
            }
        };
    }

    private Event pending;
    private java.util.List<Exception> failures;
    private java.util.concurrent.Executor executor;
    private Console console;

    void handle(Exception e) {}
}
