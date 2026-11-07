import { GLYPH, type GlyphName } from "./fluentGlyphs";

/** One of Windows 11's own icons (Segoe Fluent Icons), by name or code point. */
export function Fluent({
  glyph,
  size = 16,
  className,
}: {
  glyph: GlyphName | number;
  size?: number;
  className?: string;
}) {
  const code = typeof glyph === "number" ? glyph : GLYPH[glyph];
  return (
    <span
      className={className ? `fluent ${className}` : "fluent"}
      style={{ fontSize: size }}
      aria-hidden="true"
    >
      {String.fromCharCode(code)}
    </span>
  );
}
