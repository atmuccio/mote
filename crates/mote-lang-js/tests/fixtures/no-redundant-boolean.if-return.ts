export function isShape(value: unknown): value is Shape {
  if (value instanceof Shape) { // expect: no-redundant-boolean
    return true;
  }
  return false;
}

export class Point {
  constructor(readonly x: number, readonly y: number) {}

  equals(other: unknown): boolean {
    if (!(other instanceof Point)) {
      return false;
    }
    const that = other as Point;
    if (this.x !== that.x) {
      return false;
    }
    return true;
  }

  isOrigin(): boolean {
    if (this.x === 0 && this.y === 0) return true; // expect: no-redundant-boolean
    else return false;
  }

  hasName(name?: string): boolean {
    if (name) { // expect: no-redundant-boolean
      return true;
    }
    return false;
  }
}

export const canEdit = (user: User): boolean => {
  if (user.role === 'admin') {
    return true;
  }
  if (user.role === 'owner') {
    return true;
  }
  return false;
};

export function castLiteral(x: number): boolean {
  if (x > 0) return true as boolean; // expect: no-redundant-boolean
  return false;
}

declare class Shape {}
interface User {
  role: string;
}
