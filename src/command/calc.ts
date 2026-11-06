/**
 * The command bar's calculator: + − × ÷, powers (^ or **), %, brackets, a few functions
 * (sqrt, sin, cos, tan, log, ln, abs, round, floor, ceil) and the constants pi and e. A
 * recursive-descent parser, never `eval`.
 */

type Token =
  | { type: "num"; value: number }
  | { type: "op"; value: string }
  | { type: "name"; value: string }
  | { type: "paren"; value: "(" | ")" };

const FUNCTIONS: Record<string, (x: number) => number> = {
  sqrt: Math.sqrt,
  sin: Math.sin,
  cos: Math.cos,
  tan: Math.tan,
  log: Math.log10,
  ln: Math.log,
  abs: Math.abs,
  round: Math.round,
  floor: Math.floor,
  ceil: Math.ceil,
};
const CONSTANTS: Record<string, number> = { pi: Math.PI, π: Math.PI, e: Math.E };

function tokenize(input: string): Token[] | null {
  const text = input.replace(/×/g, "*").replace(/÷/g, "/").replace(/−/g, "-").replace(/\*\*/g, "^");
  const tokens: Token[] = [];
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    if (/\s/.test(c)) {
      i++;
    } else if (/[0-9.]/.test(c)) {
      const match = /^(\d+(?:,\d{3})*(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?/i.exec(text.slice(i));
      if (!match) return null;
      const value = Number(match[0].replace(/,/g, ""));
      if (!Number.isFinite(value)) return null;
      tokens.push({ type: "num", value });
      i += match[0].length;
    } else if ("+-*/^%".includes(c)) {
      tokens.push({ type: "op", value: c });
      i++;
    } else if (c === "(" || c === ")") {
      tokens.push({ type: "paren", value: c });
      i++;
    } else if (/[a-zπ]/i.test(c)) {
      const match = /^[a-zπ]+/i.exec(text.slice(i))!;
      const name = match[0].toLowerCase();
      if (!(name in FUNCTIONS) && !(name in CONSTANTS)) return null;
      tokens.push({ type: "name", value: name });
      i += match[0].length;
    } else {
      return null;
    }
  }
  return tokens;
}

class Parser {
  private at = 0;
  constructor(private readonly tokens: Token[]) {}

  private peek(): Token | undefined {
    return this.tokens[this.at];
  }

  private isOp(value: string): boolean {
    const t = this.peek();
    return t?.type === "op" && t.value === value;
  }

  done(): boolean {
    return this.at >= this.tokens.length;
  }

  // expr := term (("+" | "-") term)*
  expr(): number {
    let value = this.term();
    while (this.isOp("+") || this.isOp("-")) {
      const op = (this.tokens[this.at++] as { value: string }).value;
      const right = this.term();
      value = op === "+" ? value + right : value - right;
    }
    return value;
  }

  // term := unary (("*" | "/") unary | implicit-multiply)*
  private term(): number {
    let value = this.unary();
    for (;;) {
      if (this.isOp("*") || this.isOp("/")) {
        const op = (this.tokens[this.at++] as { value: string }).value;
        const right = this.unary();
        value = op === "*" ? value * right : value / right;
      } else if (this.peek()?.type === "paren" && this.peek()?.value === "(") {
        // 2(3+4)
        value *= this.unary();
      } else if (this.peek()?.type === "name") {
        // 2pi
        value *= this.unary();
      } else {
        return value;
      }
    }
  }

  // unary := ("-" | "+") unary | power
  private unary(): number {
    if (this.isOp("-")) {
      this.at++;
      return -this.unary();
    }
    if (this.isOp("+")) {
      this.at++;
      return this.unary();
    }
    return this.power();
  }

  // power := postfix ("^" unary)?   (right-associative)
  private power(): number {
    const base = this.postfix();
    if (this.isOp("^")) {
      this.at++;
      return base ** this.unary();
    }
    return base;
  }

  // postfix := atom "%"*
  private postfix(): number {
    let value = this.atom();
    while (this.isOp("%")) {
      this.at++;
      value /= 100;
    }
    return value;
  }

  private atom(): number {
    const t = this.tokens[this.at++];
    if (!t) throw new Error("end");
    if (t.type === "num") return t.value;
    if (t.type === "paren" && t.value === "(") {
      const value = this.expr();
      const close = this.tokens[this.at++];
      if (close?.type !== "paren" || close.value !== ")") throw new Error("bracket");
      return value;
    }
    if (t.type === "name") {
      if (t.value in CONSTANTS) return CONSTANTS[t.value];
      return FUNCTIONS[t.value](this.atom());
    }
    throw new Error("unexpected");
  }
}

/**
 * The value of `input` if it is a sum, else null. A lone number isn't a sum (typing "2024"
 * shouldn't offer to copy 2024), and neither is an unfinished one.
 */
export function calculate(input: string): number | null {
  const text = input.trim().replace(/=$/, "").trim();
  if (!text) return null;
  const tokens = tokenize(text);
  if (!tokens || tokens.length === 0) return null;
  const isSum = tokens.some((t) => t.type === "op" || t.type === "name" || t.type === "paren");
  if (!isSum) return null;
  // A single constant name ("e") is a word someone is typing, not a sum.
  if (tokens.length === 1) return null;
  try {
    const parser = new Parser(tokens);
    const value = parser.expr();
    if (!parser.done() || !Number.isFinite(value)) return null;
    return value;
  } catch {
    return null;
  }
}

/** A result as text to copy: up to 12 significant digits, no float noise ("0.1+0.2" → "0.3"). */
export function formatResult(value: number): string {
  if (Object.is(value, -0)) return "0";
  if (Math.abs(value) >= 1e15 || (value !== 0 && Math.abs(value) < 1e-9)) {
    return value.toExponential(6).replace(/\.?0+e/, "e");
  }
  return String(Number(value.toPrecision(12)));
}
