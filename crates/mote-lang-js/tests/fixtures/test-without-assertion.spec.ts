// mote-test: test-file
import { ComponentFixture, TestBed, fakeAsync, tick } from '@angular/core/testing';
import { HttpTestingController } from '@angular/common/http/testing';
import { ListComponent } from './list.component';
import type { DeepPartial } from './types';

describe('ListComponent', () => {
  let component: ListComponent;
  let fixture: ComponentFixture<ListComponent>;
  let httpMock: HttpTestingController;

  beforeEach(() => {
    TestBed.configureTestingModule({ declarations: [ListComponent] });
    fixture = TestBed.createComponent(ListComponent);
    component = fixture.componentInstance;
  });

  it('should init', () => { // expect: test-without-assertion
    component.ngOnInit();
    fixture.detectChanges();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });

  it('verifies requests', () => {
    component.load();
    httpMock.expectOne('/api/items').flush([]);
    httpMock.verify();
  });

  it('wraps in fakeAsync', fakeAsync(() => {
    component.load();
    tick(100);
  }));
});

describe('types', () => {
  function acceptsPartial(_: Partial<Config>): void {}

  it('accepts a partial config', () => {
    acceptsPartial({ name: 'x' });
  });

  it('rejects a wrong type', () => {
    // @ts-expect-error: number is not a Config
    const c: Config = 1;
    String(c);
  });
});

it('checks a generic type at compile time', () => {
  function build<E extends Config>(entity: DeepPartial<E>) {
    entity.name = 'x';
  }
  build({});
});
