/**
 * How well a search matches a name, for the command bar and the settings search. Higher is
 * better; 0 means no match. Whole-name, start-of-name and start-of-word matches beat matches
 * in the middle of a word, which beat initials, which beat letters in order ("vsc" → "Visual
 * Studio Code" by initials, "ntpd" → "Notepad" by letters in order).
 */

/** Lowercase, accents dropped, so "café" finds "Cafe". */
export function fold(text: string): string {
  return text
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase();
}

function words(text: string): string[] {
  return text.split(/[^\p{L}\p{N}]+/u).filter(Boolean);
}

/** Letters of `query` appear in `text` in order. Returns how spread out they are, or -1. */
function subsequenceGap(query: string, text: string): number {
  let at = 0;
  let first = -1;
  for (const ch of query) {
    const found = text.indexOf(ch, at);
    if (found < 0) return -1;
    if (first < 0) first = found;
    at = found + 1;
  }
  return at - first - query.length;
}

/** Score one query word against one name. */
function scoreWord(word: string, name: string, nameWords: string[]): number {
  if (name === word) return 100;
  if (name.startsWith(word)) return 90 - Math.min(10, name.length - word.length) * 0.5;
  if (nameWords.some((w) => w.startsWith(word))) return 75;
  const index = name.indexOf(word);
  if (index >= 0) return 55 - Math.min(15, index);
  const initials = nameWords.map((w) => w[0]).join("");
  if (word.length >= 2 && initials.startsWith(word)) return 65;
  if (word.length >= 3) {
    const gap = subsequenceGap(word, name);
    if (gap >= 0) return Math.max(5, 30 - gap * 2);
  }
  return 0;
}

/**
 * Score `query` against `name` (and optional extra words that also find it, such as
 * "dark mode" for the theme setting). Every word of the query must match something.
 */
export function score(query: string, name: string, keywords: string[] = []): number {
  const q = fold(query.trim());
  if (!q) return 0;
  const n = fold(name);
  const nWords = words(n);
  const whole = scoreWord(q, n, nWords);
  // A multi-word query that matches the name as typed ranks as one match.
  if (whole >= 55) return whole;
  const queryWords = words(q);
  if (queryWords.length === 0) return 0;
  const extra = keywords.map(fold);
  let total = 0;
  for (const word of queryWords) {
    const onName = scoreWord(word, n, nWords);
    const onKeywords = Math.max(0, ...extra.map((k) => scoreWord(word, k, words(k)) * 0.7));
    const best = Math.max(onName, onKeywords);
    if (best === 0) return 0;
    total += best;
  }
  return total / queryWords.length - (queryWords.length > 1 ? 2 : 0);
}

export interface Rankable {
  name: string;
  keywords?: string[];
  /** Added to the score, to favour some kinds of result (apps over settings, say). */
  boost?: number;
}

/** The items matching `query`, best first, at most `limit`. Ties keep their given order. */
export function rank<T extends Rankable>(query: string, items: T[], limit = 50): T[] {
  return items
    .map((item, index) => ({ item, index, s: score(query, item.name, item.keywords) }))
    .filter((r) => r.s > 0)
    .map((r) => ({ ...r, s: r.s + (r.item.boost ?? 0) }))
    .sort((a, b) => b.s - a.s || a.index - b.index)
    .slice(0, limit)
    .map((r) => r.item);
}
