import { useEffect, useState } from "react";
import { Check, KeyRound } from "lucide-react";

import { ipc } from "../../core/ipc";
import { errorText, featuresIpc } from "../../core/ipcFeatures";
import type { CardSettings } from "../../core/generated/CardSettings";
import type { Count } from "../../core/generated/Count";
import type { Service } from "../../core/generated/Service";
import { useSettings } from "../../settings/store";
import { SERVICE_NAMES } from "./accountMeta";

function report(error: unknown) {
  console.error("account card action failed", error);
}

/** Where to get each service's key, in a few steps (read-only access where there is one). */
const HELP: Record<Service, string> = {
  stripe:
    "Stripe Dashboard → Developers → API keys → Create restricted key. Give it Read access to Balance, and paste the rk_… key.",
  polar: "Polar → Settings → Developers → New token, with the metrics:read scope. Paste the polar_… token.",
  shopify:
    "Shopify admin → Settings → Apps → Develop apps → Create an app with the read_orders scope → Install → paste the Admin API token (shpat_…).",
  gumroad: "Gumroad → Settings → Advanced → Applications → create one → Generate access token.",
  paddle: "Paddle → Developer tools → Authentication → New API key, with read access to transactions.",
  lemonSqueezy: "Lemon Squeezy → Settings → API → add a key.",
  dodo: "Dodo Payments → Developer → API keys → create a key for live mode.",
  plausible: "Plausible → Account settings → API keys → New key (Stats API). Add your site's domain below.",
  datafast: "DataFast → your website's Settings → API → create a key (df_…).",
  dub: "Dub → Settings → API keys → create a key with read access to analytics (dub_…).",
  resend: "resend.com/api-keys → Create API key with Full access (Resend only lists emails for those). re_…",
  linear: "Linear → Settings → Security & access → Personal API keys → New key (lin_api_…).",
  supabase:
    "supabase.com/dashboard/account/tokens → Generate new token (sbp_…). Add your project's reference ID (Project settings → General) below.",
  youtube:
    "Google Cloud console → enable YouTube Data API v3 → Credentials → API key. Add your channel ID (UC…, from youtube.com/account_advanced) below.",
  instagram:
    "Meta for Developers → your app → Instagram API with Instagram login → Generate access token for your account.",
  x: "developer.x.com → your app → Keys and tokens → Bearer token. X only allows looking up accounts on a paid API plan.",
  deepseek: "platform.deepseek.com/api_keys → Create new API key (sk-…).",
};

/** The extra, non-secret choice some services need. */
const EXTRA: Partial<Record<Service, { field: keyof CardSettings; label: string; placeholder: string }>> = {
  shopify: { field: "shopifyStore", label: "Store address", placeholder: "your-store.myshopify.com" },
  plausible: { field: "plausibleSite", label: "Site domain", placeholder: "example.com" },
  supabase: { field: "supabaseProject", label: "Project reference ID", placeholder: "abcdefghijklmnop" },
  youtube: { field: "youtubeChannel", label: "Channel ID", placeholder: "UC…" },
  x: { field: "xUsername", label: "Username", placeholder: "@you" },
};

/** Paste (or remove) a service's key, and its extra choice. */
export function ServicePanel({ service, details }: { service: Service; details?: boolean }) {
  const [connected, setConnected] = useState<boolean | null>(null);
  const [key, setKey] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const cards = useSettings((s) => s.settings?.cards);
  const update = useSettings((s) => s.update);
  const extra = EXTRA[service];
  const [extraValue, setExtraValue] = useState(extra && cards ? String(cards[extra.field] ?? "") : "");

  useEffect(() => {
    ipc
      .serviceConnected()
      .then((list) => setConnected(list.includes(service)))
      .catch(report);
  }, [service]);

  const save = (value: string) => {
    setMessage(null);
    ipc
      .serviceKeySet(service, value)
      .then(() => {
        setConnected(value.trim() !== "");
        setKey("");
        setMessage(value.trim() ? "Saved in Windows Credential Manager." : "Removed.");
      })
      .catch((error) => setMessage(errorText(error, "Couldn't save the key.")));
  };

  return (
    <div className="panel-body card-panel">
      <h2 className="panel-title">{SERVICE_NAMES[service]}</h2>
      <p className="panel-faint">{HELP[service]}</p>
      <label className="card-field">
        <span className="panel-faint">
          {connected ? (
            <>
              <Check size={12} /> Key saved. Paste a new one to replace it.
            </>
          ) : (
            <>
              <KeyRound size={12} /> Key
            </>
          )}
        </span>
        <input
          type="password"
          value={key}
          autoComplete="off"
          spellCheck={false}
          placeholder={connected ? "••••••••" : "Paste your key"}
          onChange={(e) => setKey(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && key.trim()) save(key);
          }}
        />
      </label>
      <div className="card-field-row">
        <button className="panel-button" disabled={!key.trim()} onClick={() => save(key)}>
          Save key
        </button>
        {connected && (
          <button className="panel-link" onClick={() => save("")}>
            Remove key
          </button>
        )}
      </div>
      {extra && cards && (
        <label className="card-field">
          <span className="panel-faint">{extra.label}</span>
          <input
            value={extraValue}
            placeholder={extra.placeholder}
            spellCheck={false}
            onChange={(e) => setExtraValue(e.target.value)}
            onBlur={() => update({ cards: { ...cards, [extra.field]: extraValue } }).catch(report)}
          />
        </label>
      )}
      {message && <p className="panel-faint">{message}</p>}
      {details && connected && <ServiceDetails service={service} />}
    </div>
  );
}

/** Linear's issues, listed under its setup. */
function ServiceDetails({ service }: { service: Service }) {
  const [count, setCount] = useState<Count | null>(null);
  useEffect(() => {
    ipc.serviceCount(service).then(setCount).catch(report);
  }, [service]);
  if (!count?.items.length) return null;
  return (
    <ul className="card-panel-list">
      {count.items.map((item) => (
        <li key={item}>
          <span className="card-panel-name">{item}</span>
        </li>
      ))}
    </ul>
  );
}

/** Total Revenue: the goal and the currency. */
export function TotalRevenuePanel() {
  const cards = useSettings((s) => s.settings?.cards);
  const update = useSettings((s) => s.update);
  const [goal, setGoal] = useState(String(cards?.revenueGoal ?? 0));
  const [currency, setCurrency] = useState(cards?.revenueCurrency ?? "USD");
  if (!cards) return null;
  return (
    <div className="panel-body card-panel">
      <h2 className="panel-title">Total revenue</h2>
      <p className="panel-faint">
        Adds up every store you've connected on a revenue card (Stripe, Polar, Shopify, Gumroad, Paddle, Lemon
        Squeezy, Dodo), converted at today's rates.
      </p>
      <div className="card-field-row">
        <label className="card-field">
          <span className="panel-faint">Monthly goal</span>
          <input
            value={goal}
            inputMode="numeric"
            onChange={(e) => setGoal(e.target.value)}
            onBlur={() =>
              update({ cards: { ...cards, revenueGoal: Math.max(0, Math.round(Number(goal) || 0)) } }).catch(
                report,
              )
            }
          />
        </label>
        <label className="card-field">
          <span className="panel-faint">Currency</span>
          <input
            value={currency}
            maxLength={3}
            onChange={(e) => setCurrency(e.target.value.toUpperCase())}
            onBlur={() => update({ cards: { ...cards, revenueCurrency: currency } }).catch(report)}
          />
        </label>
      </div>
    </div>
  );
}

/** Gmail and GitHub use the accounts in Settings → Accounts. */
export function AccountsHintPanel({ which }: { which: "gmail" | "github" }) {
  return (
    <div className="panel-body card-panel">
      <h2 className="panel-title">{which === "gmail" ? "Gmail" : "GitHub"}</h2>
      <p className="panel-faint">
        {which === "gmail"
          ? "Gmail uses your Google sign-in from Settings → Accounts (the same one as Google Tasks). If you signed in before Gmail was added, sign in again so Dock may read the unread count (never your mail)."
          : "GitHub uses the personal token in Settings → Accounts (the same one as GitHub Actions). It needs the notifications and repo read scopes."}
      </p>
      <button
        className="panel-button"
        onClick={() => featuresIpc.commandOpenSettings("accounts").catch(report)}
      >
        Open Settings → Accounts
      </button>
    </div>
  );
}
