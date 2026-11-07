import type { Theme } from "../../core/generated/Theme";

/** Light or dark text on the glass. */
export type Ink = "light" | "dark";

/** Per style: whether the wallpaper shows through, how strongly the rim bends it, and whether
 *  text follows the brightness behind the glass. Tint, blur and rim live in tokens.css. */
export const GLASS_STYLES: Record<Theme, { picture: boolean; refract: number; adaptiveInk: boolean }> = {
  windows: { picture: true, refract: 0, adaptiveInk: false },
  glass: { picture: true, refract: 36, adaptiveInk: true },
  iosGlass: { picture: true, refract: 60, adaptiveInk: true },
  midnight: { picture: true, refract: 22, adaptiveInk: false },
  white: { picture: false, refract: 0, adaptiveInk: false },
  black: { picture: false, refract: 0, adaptiveInk: false },
  clear: { picture: true, refract: 30, adaptiveInk: true },
};
