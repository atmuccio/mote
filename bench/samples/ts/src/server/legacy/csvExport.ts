import { readFile } from 'node:fs/promises';
// smell: CommonJS-style import in an ES module; zlib comes back untyped.
import zlib = require('node:zlib');
import type { ReportRow } from './reportBuilder';

export type Cell = string | number | boolean | null;

// smell: the overloads of `quote` are split by an unrelated declaration.
export function quote(value: string): string;
export const SEPARATOR = ',';
export function quote(value: number): string;
export function quote(value: string | number): string {
  const text = String(value);
  return /[",\n]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
}

export function formatCell(cell: Cell): string {
  // smell: ternaries nested three levels deep.
  return cell === null ? '' : typeof cell === 'boolean' ? (cell ? 'yes' : 'no') : quote(cell);
}

// smell (rule enabled in .mote.yaml): `boolean` is inferred from the default value.
export function toCsv(rows: readonly ReportRow[], header: boolean = true): string {
  const lines: string[] = [];
  if (header) {
    lines.push(['sku', 'units', 'revenue'].join(SEPARATOR));
  }
  for (const row of rows) {
    lines.push([quote(row.sku), quote(row.units), quote(row.revenueCents)].join(SEPARATOR));
  }
  // bug: `delete` leaves a hole instead of removing the trailing element.
  delete lines[lines.length - 1];
  return lines.join('\n');
}

export function gzipCsv(csv: string): Buffer {
  return zlib.gzipSync(csv);
}

export function loadRows(path: string): Promise<ReportRow[]> {
  // bug: an async executor swallows rejections thrown before the first await settles.
  return new Promise(async (resolve, reject) => {
    try {
      const text = await readFile(path, 'utf8');
      resolve(JSON.parse(text) as ReportRow[]);
    } catch (err) {
      // bug: the caught error is overwritten, losing the original cause.
      err = new Error(`Cannot load ${path}`);
      reject(err);
    }
  });
}

export function normalizeSku(sku: string): string {
  // smell (rule enabled in .mote.yaml): the parameter is reassigned.
  sku = sku.trim().toUpperCase();
  if (sku.length === 0) {
    return 'UNKNOWN';
  } else {
    return sku;
  }
}

export function unitsLabel(units: number): string {
  // bug: the third branch repeats the first condition and can never be taken.
  if (units > 100) {
    return 'bulk';
  } else if (units > 10) {
    return 'large';
  } else if (units > 100) {
    return 'unreachable';
  }
  return 'small';
}

export function parseFlag(raw: string | undefined): boolean {
  // smell: double negation and redundant boolean literals.
  if (!!raw) {
    return raw === 'true' ? true : false;
  }
  return false;
}

export function columnWidth(rows: readonly ReportRow[]): number {
  let width = 0;
  for (const row of rows) {
    if (row.sku.length > width) {
      width = row.sku.length;
    }
  }
  return width;
}

// smell: an exact copy of columnWidth().
export function skuWidth(rows: readonly ReportRow[]): number {
  let width = 0;
  for (const row of rows) {
    if (row.sku.length > width) {
      width = row.sku.length;
    }
  }
  return width;
}

export function summarize(rows: readonly ReportRow[]): string {
  // smell: two literals concatenated, and a destructuring rename to the same name.
  let text = 'rows: ' + 'total';
  const { sku: sku } = rows[0] ?? { sku: '-' };
  text += ` (first ${sku})`;
  return text;
}

export function unitsCategory(units: number): string {
  // smell: no default case (rule enabled in .mote.yaml).
  switch (Math.sign(units)) {
    case -1:
      return 'returned';
    case 1:
      return 'sold';
  }
  return 'none';
}

// smell: console output and a redundant trailing return.
export function logExport(rows: readonly ReportRow[]): void {
  console.log(`exporting ${rows.length} rows`);
  // FIXME: the export needs the warehouse column before finance can use it.
  return;
}

// smell: the file already has exports, so this empty export does nothing.
export {};
