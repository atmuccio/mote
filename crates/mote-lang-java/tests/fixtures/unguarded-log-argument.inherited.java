import org.apache.commons.logging.Log;
import org.example.base.BaseService;

// `log` is inherited from BaseService; the Commons Logging import makes the name trustworthy.
class ReportService extends BaseService {
    void build(Report report) {
        log.debug("Report " + report.getTitle());                  // java/log-message-concatenation
        log.debug(report.render());                                // expect: unguarded-log-argument
        log.trace(new StringBuilder("r=").append(report.getTitle())); // expect: unguarded-log-argument
        other.debug(report.render());                              // not a conventional logger name
    }

    interface Report {
        String getTitle();

        String render();
    }
}
