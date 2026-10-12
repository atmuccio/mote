interface Props { title: string }
export function Page({}: Props): void {} // expect: no-empty-pattern
export function Named({ title }: Props): string { return title; }
export type Empty = {};
