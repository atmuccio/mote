type Id = string; // expect: invisible-character
export const label: Id = 'id​'; // expect: invisible-character
export type Unit = 'kg' | ' g'; // expect: invisible-character
export const ok: Id = 'id\u200b';
export const unit: Unit = 'kg';
