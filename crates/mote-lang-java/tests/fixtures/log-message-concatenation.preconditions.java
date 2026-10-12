import static com.google.common.base.Preconditions.checkArgument;
import static com.google.common.base.Preconditions.checkState;

import com.google.common.base.Preconditions;
import java.util.Objects;
import org.apache.commons.lang3.Validate;
import org.springframework.util.Assert;

class Service {
    private static final String PREFIX = "service: ";

    void positives(Object config, String name, int size) {
        Objects.requireNonNull(config, "no config for " + name);                  // expect: log-message-concatenation
        checkArgument(size > 0, "bad size " + size);                              // expect: log-message-concatenation
        checkState(name != null, String.format("state %s", name));                // expect: log-message-concatenation
        Preconditions.checkNotNull(config, "missing " + name);                    // expect: log-message-concatenation
        Assert.notNull(config, "config of " + name + " is required");             // expect: log-message-concatenation
        Assert.isInstanceOf(String.class, config, "not a string: " + config);     // expect: log-message-concatenation
        Validate.isTrue(size < 10, "too big: " + size);                           // expect: log-message-concatenation
    }

    void negatives(Object config, String name, int size, Other other) {
        Objects.requireNonNull(config, () -> "no config for " + name);
        Objects.requireNonNull(config, "no config");
        Objects.requireNonNull(config, PREFIX + "config");
        checkArgument(size > 0, "bad size %s", size);
        Assert.notNull(config, () -> "config of " + name);
        Validate.isTrue(size < 10, "too big: %d", size);
        other.requireNonNull(config, "x" + name);                                 // not a known class
        Assert.state(size > 0, "size");
        log.info("config " + name);                                               // other log levels stay out
    }

    private final org.slf4j.Logger log = org.slf4j.LoggerFactory.getLogger(Service.class);
}

class OwnHelper {
    void run(String name) {
        checkArgument(name.isEmpty(), "bad " + name);                             // this file's own helper
    }

    static void checkArgument(boolean ok, String message) { }
}
