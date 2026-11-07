import { Fluent } from "../core/Fluent";
import { useInputLanguage } from "../core/system";

/** The taskbar's ^: Windows' flyout of tray icons that don't fit. */
export function HiddenIconsTile() {
  return (
    <span className="tray-glyph">
      <Fluent glyph="chevronUp" size={14} />
    </span>
  );
}

/** The input language, two lines like the taskbar's (ENG over IN). */
export function LanguageTile() {
  const language = useInputLanguage();
  return (
    <span className="tray-language">
      <span>{language?.language || "ENG"}</span>
      {language?.region && <span>{language.region}</span>}
    </span>
  );
}

/** The notification bell. */
export function NotificationsTile() {
  return (
    <span className="tray-glyph">
      <Fluent glyph="bell" />
    </span>
  );
}

/** These open Windows' own flyouts, never a dock popup. */
export function NoPanel() {
  return null;
}
