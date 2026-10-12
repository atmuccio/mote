enum Kind {
  A,
  B,
  C,
}

export function describe(kind: Kind, value: string | number): string {
  switch (kind) {
    case Kind.A:
      return format(value as string, {
        upper: true,
      });
    case Kind.B: // expect: no-duplicated-branches
      return format(value as string, {
        upper: true,
      });
    case Kind.C:
      return format(value as string, {
        upper: false,
      });
  }
}

export function typed(value: string | number): void {
  if (typeof value === 'string') {
    emit(value as never);
    done();
  } else { // expect: no-duplicated-branches
    emit(value as never);
    done();
  }
  if (typeof value === 'string') {
    emit(value as string);
    done();
  } else {
    emit(value as number as never);
    done();
  }
}

declare function format(value: string, options: { upper: boolean }): string;
declare function emit(value: never): void;
declare function done(): void;
