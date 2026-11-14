import type { Service } from "../../core/generated/Service";

/** A service's mark: its colour and a letter (no logos). */
export const MARKS: Record<Service, { color: string; letter: string; ink?: string }> = {
  stripe: { color: "#635bff", letter: "S" },
  polar: { color: "#0062ff", letter: "P" },
  shopify: { color: "#95bf47", letter: "S" },
  gumroad: { color: "#ff90e8", letter: "G", ink: "#000" },
  paddle: { color: "#fddd35", letter: "P", ink: "#000" },
  lemonSqueezy: { color: "#ffc233", letter: "L", ink: "#3b2a00" },
  dodo: { color: "#0f9d58", letter: "D" },
  plausible: { color: "#5850ec", letter: "P" },
  datafast: { color: "#f97316", letter: "D" },
  dub: { color: "#111111", letter: "d" },
  resend: { color: "#111111", letter: "R" },
  linear: { color: "#5e6ad2", letter: "L" },
  supabase: { color: "#3ecf8e", letter: "S", ink: "#062c1c" },
  youtube: { color: "#ff0000", letter: "" },
  instagram: { color: "#d6249f", letter: "" },
  x: { color: "#000000", letter: "𝕏" },
  deepseek: { color: "#4d6bfe", letter: "D" },
};

export const SERVICE_NAMES: Record<Service, string> = {
  stripe: "Stripe",
  polar: "Polar",
  shopify: "Shopify",
  gumroad: "Gumroad",
  paddle: "Paddle",
  lemonSqueezy: "Lemon Squeezy",
  dodo: "Dodo Payments",
  plausible: "Plausible",
  datafast: "DataFast",
  dub: "Dub",
  resend: "Resend",
  linear: "Linear",
  supabase: "Supabase",
  youtube: "YouTube",
  instagram: "Instagram",
  x: "X",
  deepseek: "DeepSeek",
};
