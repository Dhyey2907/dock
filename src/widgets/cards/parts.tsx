import type { KeyboardEvent, MouseEvent, PointerEvent, ReactNode } from "react";

/**
 * Building blocks for the Lumabar-style card widgets. Sizes are in "card units": 1cu is a
 * 64th of the card's height (--cu, set by the dock), so a card looks the same at any dock size.
 */

/** A card's content area. `wide` cards stretch further. */
export function Card({
  children,
  className = "",
  tone,
}: {
  children: ReactNode;
  className?: string;
  tone?: string;
}) {
  return (
    <span className={`card ${className}`} data-tone={tone}>
      {children}
    </span>
  );
}

/** A control inside a card: a click on it doesn't open the card's popup or start a drag. */
export function CardButton({
  label,
  onPress,
  className = "",
  children,
  active,
}: {
  label: string;
  onPress: () => void;
  className?: string;
  children: ReactNode;
  active?: boolean;
}) {
  const stop = (e: MouseEvent | PointerEvent) => e.stopPropagation();
  return (
    <span
      role="button"
      tabIndex={-1}
      aria-label={label}
      title={label}
      className={`card-button ${className}`}
      data-active={active}
      onPointerDown={stop}
      onPointerUp={stop}
      onClick={(e) => {
        e.stopPropagation();
        onPress();
      }}
      onKeyDown={(e: KeyboardEvent) => {
        if (e.key === "Enter" || e.key === " ") {
          e.stopPropagation();
          onPress();
        }
      }}
    >
      {children}
    </span>
  );
}

/** A progress ring, `value` 0–100. */
export function Ring({
  value,
  size = 44,
  stroke = 5,
  color = "var(--card-accent)",
  children,
  gap = 0,
}: {
  value: number;
  size?: number;
  stroke?: number;
  color?: string;
  children?: ReactNode;
  /** A gap at the bottom (degrees), for gauges. */
  gap?: number;
}) {
  const r = 50 - stroke;
  const circumference = 2 * Math.PI * r;
  const usable = circumference * (1 - gap / 360);
  const filled = (usable * Math.max(0, Math.min(100, value))) / 100;
  const rotate = 90 + gap / 2;
  return (
    <span
      className="card-ring"
      style={{ width: `calc(${size} * var(--cu))`, height: `calc(${size} * var(--cu))` }}
    >
      <svg viewBox="0 0 100 100" aria-hidden="true">
        <circle
          cx="50"
          cy="50"
          r={r}
          className="card-ring-track"
          strokeWidth={stroke * 2}
          strokeDasharray={`${usable} ${circumference}`}
          transform={`rotate(${rotate} 50 50)`}
        />
        {filled > 0.5 && (
          <circle
            cx="50"
            cy="50"
            r={r}
            fill="none"
            stroke={color}
            strokeWidth={stroke * 2}
            strokeLinecap="round"
            strokeDasharray={`${filled} ${circumference}`}
            transform={`rotate(${rotate} 50 50)`}
          />
        )}
      </svg>
      {children && <span className="card-ring-label">{children}</span>}
    </span>
  );
}

/** A thin horizontal bar, `value` 0–100. */
export function Meter({
  value,
  color = "var(--card-accent)",
  width,
}: {
  value: number;
  color?: string;
  width?: number;
}) {
  return (
    <span className="card-meter" style={width ? { width: `calc(${width} * var(--cu))` } : undefined}>
      <span style={{ width: `${Math.max(0, Math.min(100, value))}%`, background: color }} />
    </span>
  );
}

/** A smooth line through `points` (any scale), with an optional fill under it. */
export function Spark({
  points,
  width = 120,
  height = 30,
  color = "var(--card-accent)",
  fill = false,
}: {
  points: number[];
  width?: number;
  height?: number;
  color?: string;
  fill?: boolean;
}) {
  if (points.length < 2) {
    return (
      <span
        className="card-spark"
        style={{ width: `calc(${width} * var(--cu))`, height: `calc(${height} * var(--cu))` }}
      />
    );
  }
  const min = Math.min(...points);
  const max = Math.max(...points);
  const span = max - min || 1;
  const xy = points.map((p, i) => [(i / (points.length - 1)) * 100, 28 - ((p - min) / span) * 26]);
  const line = xy.map(([x, y], i) => `${i ? "L" : "M"}${x.toFixed(2)},${y.toFixed(2)}`).join(" ");
  return (
    <svg
      className="card-spark"
      viewBox="0 0 100 30"
      preserveAspectRatio="none"
      style={{ width: `calc(${width} * var(--cu))`, height: `calc(${height} * var(--cu))` }}
      aria-hidden="true"
    >
      {fill && <path d={`${line} L100,30 L0,30 Z`} fill={color} opacity="0.18" />}
      <path
        d={line}
        fill="none"
        stroke={color}
        strokeWidth="2"
        vectorEffect="non-scaling-stroke"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/** Small vertical bars (screen time by hour). */
export function Bars({
  values,
  width = 110,
  height = 28,
  color = "var(--card-accent)",
  mark,
}: {
  values: number[];
  width?: number;
  height?: number;
  color?: string;
  /** Index drawn brighter (now). */
  mark?: number;
}) {
  const max = Math.max(1, ...values);
  return (
    <span
      className="card-bars"
      style={{ width: `calc(${width} * var(--cu))`, height: `calc(${height} * var(--cu))` }}
    >
      {values.map((v, i) => (
        <span
          key={i}
          data-empty={v === 0}
          data-mark={i === mark}
          style={{
            height: v === 0 ? undefined : `${Math.max(8, (v / max) * 100)}%`,
            background: v ? color : undefined,
          }}
        />
      ))}
    </span>
  );
}

/** A grid of dots or squares, `filled` of `total` lit (or each cell's level 0–4). */
export function DotGrid({
  total,
  filled,
  columns,
  levels,
  square = false,
  cell = 7,
}: {
  total: number;
  filled?: number;
  columns: number;
  levels?: number[];
  square?: boolean;
  cell?: number;
}) {
  return (
    <span
      className="card-dots"
      data-square={square}
      style={{
        gridTemplateColumns: `repeat(${columns}, calc(${cell} * var(--cu)))`,
        gridAutoFlow: levels ? "column" : "row",
      }}
    >
      {Array.from({ length: total }, (_, i) => (
        <span key={i} data-level={levels ? (levels[i] ?? 0) : i < (filled ?? 0) ? 4 : 0} />
      ))}
    </span>
  );
}

/** An app-style rounded square holding an icon or a glyph. */
export function IconTile({ children, color }: { children: ReactNode; color: string }) {
  return (
    <span className="card-icon-tile" style={{ background: color }}>
      {children}
    </span>
  );
}
