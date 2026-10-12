export function connect(port: number = 8080, host: string): void {} // expect: default-param-last
export function optional(port = 8080, host?: string): void {}
export class Service {
  constructor(private readonly retries = 3, private readonly http: Http) {} // expect: default-param-last
}
export function withThis(this: Window, a = 1, b?: number) {}
export class Contribution {
  constructor(options = {}, @ILogService private readonly log: ILogService) {}
}
export class Controller {
  find(@Query('kind') kind = 'all', @Body() body: Body) {}
  mixed(limit = 10, plain: string, @Inject(X) x: X) {} // expect: default-param-last
}
