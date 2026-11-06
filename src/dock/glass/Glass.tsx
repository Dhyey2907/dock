import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";

import type { Theme } from "../../core/generated/Theme";
import { brightnessBehind, LIGHT_BACKGROUND, pictureLayout, type GlassBox } from "./placement";
import { lensMapUrl } from "./refraction";
import { useGlass } from "./store";
import { GLASS_STYLES, type Ink } from "./styles";

/** How far the picture extends past the glass, so blur and refraction never run out of it. */
const MARGIN = 16;

/**
 * The glass behind the dock's buttons: the slice of the desktop behind the dock, bent at the
 * rim by a lens map, frosted and tinted per style, with a bright edge. It only ever shows the
 * wallpaper — windows never sit behind the dock's strip of screen.
 */
export function Glass({
  theme,
  corners,
  onInk,
}: {
  theme: Theme;
  /** Corner roundness, 0 (square) to 1 (a full pill). */
  corners: number;
  onInk: (ink: Ink) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState<GlassBox | null>(null);
  const frame = useGlass((s) => s.frame);
  const geometry = useGlass((s) => s.geometry);
  const style = GLASS_STYLES[theme];
  const filterId = `dock-refract-${useId().replace(/[^a-zA-Z0-9-]/g, "")}`;

  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    const measure = () => {
      const r = element.getBoundingClientRect();
      setBox((old) =>
        old && old.left === r.left && old.top === r.top && old.width === r.width && old.height === r.height
          ? old
          : { left: r.left, top: r.top, width: r.width, height: r.height },
      );
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    window.addEventListener("resize", measure);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, [geometry]);

  const lens = useMemo(() => {
    if (!box || !style.refract || box.width < 1 || box.height < 1) return null;
    const width = box.width + 2 * MARGIN;
    const height = box.height + 2 * MARGIN;
    const url = lensMapUrl({
      width,
      height,
      inset: MARGIN,
      radius: (box.height / 2) * corners,
      bezel: Math.min(18, box.height * 0.32),
    });
    return url ? { url, width, height } : null;
  }, [box, corners, style.refract]);

  const ink: Ink = useMemo(() => {
    if (!style.adaptiveInk || !frame || !geometry || !box) return "light";
    return brightnessBehind(frame, geometry, box) > LIGHT_BACKGROUND ? "dark" : "light";
  }, [style.adaptiveInk, frame, geometry, box]);
  useEffect(() => onInk(ink), [ink, onInk]);

  const showPicture = style.picture && frame && geometry && box;
  const layout = showPicture ? pictureLayout(geometry, box, MARGIN) : null;

  return (
    <div className="glass" ref={ref} aria-hidden="true" data-picture={Boolean(layout)}>
      {layout && frame && (
        <div
          className="glass-picture"
          data-testid="glass-picture"
          style={{
            inset: -MARGIN,
            backgroundImage: `url(${frame.image})`,
            backgroundSize: `${layout.width}px ${layout.height}px`,
            backgroundPosition: `${layout.x}px ${layout.y}px`,
            filter: `${lens ? `url(#${filterId}) ` : ""}var(--glass-filter)`,
          }}
        />
      )}
      {lens && (
        <svg className="glass-defs" width="0" height="0" focusable="false">
          <filter
            id={filterId}
            x="0"
            y="0"
            width={lens.width}
            height={lens.height}
            filterUnits="userSpaceOnUse"
            primitiveUnits="userSpaceOnUse"
            colorInterpolationFilters="sRGB"
          >
            <feImage
              href={lens.url}
              x="0"
              y="0"
              width={lens.width}
              height={lens.height}
              preserveAspectRatio="none"
              result="lens"
            />
            <feDisplacementMap
              in="SourceGraphic"
              in2="lens"
              scale={style.refract}
              xChannelSelector="R"
              yChannelSelector="G"
            />
          </filter>
        </svg>
      )}
      <div className="glass-tint" />
      <div className="glass-rim" />
    </div>
  );
}
