// mote-test: params allow=defaultOption,dependencies
interface SeriesOption {
  z: number;
}

export class BarSeries {
  static type = 'series.bar';
  static dependencies = ['grid', 'polar'];
  static defaultOption: SeriesOption = { z: 2 };
  static registry = new Map<string, SeriesOption>(); // expect: mutable-public-static-field
}
