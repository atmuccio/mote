export class Scheduler {
  private timers: number[] = [];

  schedule(jobs: Array<() => void>, delays: number[]): void {
    for (var index = 0; index < jobs.length; index++) {
      this.timers.push(
        window.setTimeout(() => jobs[index](), delays[index]), // expect: no-loop-func
      );
    }
  }

  subscribeAll(sources: Array<{ subscribe(fn: (v: number) => void): void }>): void {
    for (var s = 0; s < sources.length; s++) {
      sources[s].subscribe((v: number) => this.record(s, v)); // expect: no-loop-func
    }
  }

  safe(jobs: Array<() => void>): void {
    for (const job of jobs) {
      setTimeout(() => job(), 0);
    }
    jobs.forEach((job, i) => setTimeout(() => job.call(null, i)));
  }

  private record(source: number, value: number): void {
    console.log(source, value);
  }
}
