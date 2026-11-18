import { useEffect, useState } from "react";
import { Check, KeyRound } from "lucide-react";

import { announceServicesChanged, ipc } from "../../core/ipc";
import { errorText, featuresIpc } from "../../core/ipcFeatures";
import type { Count } from "../../core/generated/Count";
import type { Revenue } from "../../core/generated/Revenue";
import type { Traffic } from "../../core/generated/Traffic";
import type { Service } from "../../core/generated/Service";
import { useSettings } from "../../settings/store";
import { Spark } from "./parts";
import { EXTRA, SERVICE_NAMES } from "./accountMeta";

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

/** Paste (or remove) a service's key, and its extra choice. */
export function ServicePanel({ service }: { service: Service; details?: boolean }) {
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
        announceServicesChanged().catch(report);
      })
      .catch((error) => setMessage(errorText(error, "Couldn't save the key.")));
  };

  return (
    <div className="panel-body card-panel">
      <h2 className="panel-title">{SERVICE_NAMES[service]}</h2>
      {connected && <ServiceDetails service={service} extra={extraValue} />}
      {connected && <h3 className="card-panel-subtitle">Key</h3>}
      {!connected && <p className="panel-faint">{HELP[service]}</p>}
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
            onBlur={() =>
              extraValue !== String(cards[extra.field] ?? "") &&
              update({ cards: { ...cards, [extra.field]: extraValue } }).catch(report)
            }
          />
        </label>
      )}
      {message && <p className="panel-faint">{message}</p>}
    </div>
  );
}

const REVENUE: Service[] = ["stripe", "polar", "shopify", "gumroad", "paddle", "lemonSqueezy", "dodo"];
const TRAFFIC: Service[] = ["plausible", "datafast", "dub"];

const money = (n: number, currency: string) => {
  try {
    return new Intl.NumberFormat(undefined, { style: "currency", currency, maximumFractionDigits: 2 }).format(
      n,
    );
  } catch {
    return `${n.toFixed(2)} ${currency}`;
  }
};
const whole = (n: number) => new Intl.NumberFormat().format(Math.round(n));
const sum = (list: number[]) => list.reduce((a, b) => a + b, 0);

/** What the service's cards show, in more detail: the reason the popup opens once it's set up. */
function ServiceDetails({ service, extra }: { service: Service; extra: string }) {
  const [state, setState] = useState<{ data?: Revenue | Traffic | Count; error?: string }>({});
  useEffect(() => {
    let alive = true;
    const read: Promise<Revenue | Traffic | Count> = REVENUE.includes(service)
      ? ipc.serviceRevenue(service)
      : TRAFFIC.includes(service)
        ? ipc.serviceTraffic(service)
        : ipc.serviceCount(service);
    read
      .then((data) => alive && setState({ data }))
      .catch(
        (error) =>
          alive &&
          setState({
            error: typeof error === "string" ? error : errorText(error, "Couldn't load this right now."),
          }),
      );
    return () => {
      alive = false;
    };
  }, [service, extra]);

  const { data, error } = state;
  if (error) return <p className="prod-problem">{error}</p>;
  if (!data) return <p className="panel-faint">Loading…</p>;

  if ("total" in data) {
    const days = data.daily.slice(-14);
    const week = sum(days.slice(7));
    const before = sum(days.slice(0, 7));
    return (
      <div className="card-panel-stats">
        <Stat label="Last 30 days" value={money(data.total, data.currency)} />
        <Stat label="Last 7 days" value={money(week, data.currency)} change={change(week, before)} />
        <Stat label="Best day" value={money(Math.max(0, ...data.daily), data.currency)} />
        {data.daily.length > 1 && (
          <div className="card-panel-chart">
            <Spark points={data.daily} width={300} height={60} color="#30d158" fill />
            <span className="panel-faint">Daily, last {data.daily.length} days</span>
          </div>
        )}
      </div>
    );
  }
  if ("week" in data) {
    const unit = service === "dub" ? "Clicks" : "Visitors";
    return (
      <div className="card-panel-stats">
        <Stat
          label={`${unit} this week`}
          value={whole(data.week)}
          change={change(data.week, data.previous)}
        />
        <Stat label="Last week" value={whole(data.previous)} />
        {data.daily.length > 1 && (
          <div className="card-panel-chart">
            <Spark points={data.daily} width={300} height={60} color="#64d2ff" fill />
            <span className="panel-faint">Daily, last {data.daily.length} days</span>
          </div>
        )}
      </div>
    );
  }
  const labels: Partial<Record<Service, [string, string?]>> = {
    resend: ["Emails sent, last 7 days", "Delivered"],
    linear: ["Issues assigned to you", "In progress"],
    supabase: ["API requests today"],
    youtube: ["Subscribers"],
    instagram: ["Followers"],
    x: ["Followers"],
    deepseek: ["Balance"],
  };
  const [first, second] = labels[service] ?? ["Total"];
  return (
    <div className="card-panel-stats">
      <Stat
        label={first}
        value={
          service === "deepseek" ? money(data.value, data.note === "CNY" ? "CNY" : "USD") : whole(data.value)
        }
      />
      {second && data.second != null && <Stat label={second} value={whole(data.second)} />}
      {data.note && NOTE_LABEL[service] && <Stat label={NOTE_LABEL[service]} value={data.note} />}
      {data.items.length > 0 && (
        <ul className="card-panel-list">
          {data.items.map((item) => (
            <li key={item}>
              <span className="card-panel-name">{item}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** What a count's note is, for the services that send one worth showing. */
const NOTE_LABEL: Partial<Record<Service, string>> = {
  supabase: "Status",
  youtube: "Channel",
  instagram: "Account",
  x: "Account",
};

function change(now: number, before: number) {
  return before ? ((now - before) / Math.abs(before)) * 100 : undefined;
}

function Stat({ label, value, change }: { label: string; value: string; change?: number }) {
  return (
    <div className="card-panel-stat">
      <span className="panel-faint">{label}</span>
      <span className="card-panel-stat-value card-tabular">
        {value}
        {change != null && (
          <span className={change >= 0 ? "card-green" : "card-red"}>
            {" "}
            {change >= 0 ? "▲" : "▼"} {Math.abs(change).toFixed(0)}%
          </span>
        )}
      </span>
    </div>
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
