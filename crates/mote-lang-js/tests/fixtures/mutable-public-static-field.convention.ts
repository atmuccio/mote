interface SeriesOption {
  z: number;
}

// Library conventions are reported unless listed in `allow`.
export class LineSeries {
  static dependencies = ['grid']; // expect: mutable-public-static-field
  static defaultOption: SeriesOption = { z: 2 }; // expect: mutable-public-static-field
  static readonly defaults: Readonly<SeriesOption> = { z: 1 };
}
