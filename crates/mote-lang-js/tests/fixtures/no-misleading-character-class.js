export function positives(s) {
  const emoji = /^[👍]$/.test(s); // expect: no-misleading-character-class
  const escapedPair = /[\uD83D\uDC4D]/; // expect: no-misleading-character-class
  const accent = /[e\u0301]/u; // expect: no-misleading-character-class
  const rawAccent = /[é]/; // expect: no-misleading-character-class
  const skinTone = /[👍🏽]/u; // expect: no-misleading-character-class
  const flag = /[🇯🇵]/u; // expect: no-misleading-character-class
  const family = /[👨‍👩‍👦]/u; // expect: no-misleading-character-class
  const heart = /[❤️]/u; // expect: no-misleading-character-class
  return [emoji, escapedPair, accent, rawAccent, skinTone, flag, family, heart];
}

// ---- compliant ----
export function negatives(s) {
  const emoji = /^[👍]$/u.test(s);
  const alternation = /^(?:👍🏽|👍)$/u;
  const accentRange = /[\u0300-\u036f]/g;
  const marksOnly = /[\u0300\u0301]/;
  const surrogateRanges = /[\uD800-\uDBFF][\uDC00-\uDFFF]/;
  const precomposed = /[é]/;
  const zeroWidth = /[\u200B-\u200D\uFEFF]/g;
  const ascii = /[a-z0-9_-]/i;
  const outside = /👍🏽/u;
  return [emoji, alternation, accentRange, marksOnly, surrogateRanges, precomposed, zeroWidth, ascii, outside];
}
