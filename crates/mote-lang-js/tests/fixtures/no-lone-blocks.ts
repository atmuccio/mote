function typed(): void {
  {
    type Local = string;
    const v: Local = 'a';
    use(v);
  }
  {
    interface I { a: number }
    use({} as I);
  }
  {
    enum E { A }
    use(E.A);
  }
  { // expect: no-lone-blocks
    use(1 as number);
  }
}
namespace N {
  { // expect: no-lone-blocks
    run();
  }
}
