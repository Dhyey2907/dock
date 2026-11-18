import type { Quote } from "../../core/generated/Quote";

/** Today's change of a quote, in percent. */
export function change(q: Quote) {
  return q.previousClose ? ((q.price - q.previousClose) / q.previousClose) * 100 : 0;
}

export const money = (n: number, currency = "USD") => {
  try {
    return new Intl.NumberFormat(undefined, {
      style: "currency",
      currency,
      maximumFractionDigits: n >= 1000 ? 0 : 2,
    }).format(n);
  } catch {
    return `${n.toFixed(2)} ${currency}`;
  }
};

/** Well-known coins' ticker and colour; any other CoinGecko id gets its name and a neutral tile. */
export const COINS: Record<string, { symbol: string; color: string }> = {
  bitcoin: { symbol: "BTC", color: "#f7931a" },
  ethereum: { symbol: "ETH", color: "#627eea" },
  solana: { symbol: "SOL", color: "#9945ff" },
  dogecoin: { symbol: "DOGE", color: "#c2a633" },
  ripple: { symbol: "XRP", color: "#23292f" },
  cardano: { symbol: "ADA", color: "#0033ad" },
  litecoin: { symbol: "LTC", color: "#345d9d" },
  binancecoin: { symbol: "BNB", color: "#f3ba2f" },
  tether: { symbol: "USDT", color: "#26a17b" },
};

/** "BTC", or "Avalanche 2" for an id we don't know. */
export function coinLabel(id: string) {
  return COINS[id]?.symbol ?? id.replace(/-/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}
