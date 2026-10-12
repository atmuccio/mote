interface ILogger {
  log(message: string): void;
}
declare abstract class AbstractLogger {
  protected abstract write(message: string): void;
}
declare namespace logging {
  interface Logger {
    log(message: string): void;
  }
}

// A class that is itself a console-backed logger: writing to the console is its job.
export class FormattedConsoleLogger extends AbstractLogger {
  protected write(message: string): void {
    for (const line of message.split('\n')) {
      console.warn(line);
    }
    [message].forEach((m) => console.error(m));
  }
}

export class PlainTarget implements ILogger {
  log(message: string): void {
    console.log(message);
  }
}

export class QualifiedTarget implements logging.Logger {
  log(message: string): void {
    console.info(message);
  }
}

export class ConsoleLog {
  write(message: string): void {
    console.log(message);
  }
}

export class DebugConsole {
  print(message: string): void {
    console.debug(message);
  }
}

// Classes that merely log are still reported.
export class LoggingInterceptor {
  intercept(message: string): void {
    console.log(message); // expect: no-console
  }
}

export class LoggerMiddleware {
  use(message: string): void {
    console.log(message); // expect: no-console
  }
}

export class Dialog {
  open(): void {
    console.log('open'); // expect: no-console
  }
}

export class Service {
  private readonly logger = new (class Inner {
    log(message: string): void {
      console.log(message); // expect: no-console
    }
  })();

  run(): void {
    this.logger.log('x');
    console.log('run'); // expect: no-console
  }
}

export function consoleLogger(message: string): void {
  console.log(message); // expect: no-console
}
