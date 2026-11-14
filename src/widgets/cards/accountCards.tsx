import { useEffect, useRef, useState, type ReactNode } from "react";
import { Bell, GitPullRequest, Mail, Play, Star } from "lucide-react";

import { ipc } from "../../core/ipc";
import { errorText } from "../../core/ipcFeatures";
import type { Count } from "../../core/generated/Count";
import type { Revenue } from "../../core/generated/Revenue";
import type { Service } from "../../core/generated/Service";
import type { Traffic } from "../../core/generated/Traffic";
import { useSettings } from "../../settings/store";
import { MARKS, SERVICE_NAMES } from "./accountMeta";
import { Card, IconTile, Ring, Spark } from "./parts";

/** How often account cards ask again (the backend also keeps answers for ten minutes). */
const EVERY = 600_000;

function Mark({ service }: { service: Service }) {
  const mark = MARKS[service];
  return (
    <IconTile color={mark.color}>
      {service === "youtube" ? (
        <Play fill="#fff" color="#fff" />
      ) : (
        <span className="card-mark" style={{ color: mark.ink ?? "#fff" }}>
          {mark.letter}
        </span>
      )}
    </IconTile>
  );
}

/** Read `read` now and every `every` ms; keeps the error's words for the card. */
function useAsk<T>(key: string, read: () => Promise<T>, every = EVERY) {
  const [state, setState] = useState<{ data?: T; error?: string }>({});
  const reader = useRef(read);
  useEffect(() => {
    reader.current = read;
  });
  useEffect(() => {
    let alive = true;
    const run = () =>
      reader
        .current()
        .then((data) => alive && setState({ data }))
        .catch(
          (error) =>
            alive &&
            setState({
              error: typeof error === "string" ? error : errorText(error, "Couldn't load this right now."),
            }),
        );
    run();
    const timer = window.setInterval(run, every);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [key, every]);
  return state;
}

const money = (n: number, currency: string) => {
  try {
    return new Intl.NumberFormat(undefined, {
      style: "currency",
      currency,
      maximumFractionDigits: Math.abs(n) >= 1000 ? 0 : 2,
    }).format(n);
  } catch {
    return `${Math.round(n)} ${currency}`;
  }
};

const compact = (n: number) =>
  new Intl.NumberFormat(undefined, { notation: "compact", maximumFractionDigits: 1 }).format(n);

function Change({ now, before }: { now: number; before: number }) {
  if (!before) return null;
  const change = ((now - before) / Math.abs(before)) * 100;
  return (
    <span className={change >= 0 ? "card-green card-small" : "card-red card-small"}>
      {change >= 0 ? "▲" : "▼"} {Math.abs(change).toFixed(0)}%
    </span>
  );
}

/** What a card shows before it has an answer: loading, or why not (no key yet…). */
function Waiting({ service, error, children }: { service: Service; error?: string; children?: ReactNode }) {
  const needsKey = error?.includes("Add your") || error?.includes("key");
  return (
    <Card>
      <Mark service={service} />
      <span className="card-col card-text">
        <span className="card-label">{SERVICE_NAMES[service]}</span>
        <span className="card-faint card-line">
          {error ? (needsKey ? "Click to connect" : error) : "Loading…"}
        </span>
      </span>
      {children}
    </Card>
  );
}

/** "Stripe": the last 30 days' net revenue. */
export function RevenueCard({ service }: { service: Service }) {
  const { data, error } = useAsk<Revenue>(`revenue/${service}`, () => ipc.serviceRevenue(service));
  if (!data) return <Waiting service={service} error={error} />;
  return (
    <Card>
      <Mark service={service} />
      <span className="card-col">
        <span className="card-label card-dim">Net revenue</span>
        <span className="card-big card-tabular">{money(data.total, data.currency)}</span>
        <span className="card-faint card-small">last 30 days</span>
      </span>
    </Card>
  );
}

/** "Stripe Chart": this week's revenue over a 14-day line, and its change on last week. */
export function RevenueChartCard({ service }: { service: Service }) {
  const { data, error } = useAsk<Revenue>(`revenue/${service}`, () => ipc.serviceRevenue(service));
  if (!data) return <Waiting service={service} error={error} />;
  const days = data.daily.slice(-14);
  const week = days.slice(7).reduce((a, b) => a + b, 0);
  const before = days.slice(0, 7).reduce((a, b) => a + b, 0);
  return (
    <Card className="card-wide card-chart">
      <span className="card-row card-between">
        <span className="card-row">
          <span className="card-dot" style={{ background: MARKS[service].color }} />
          <span className="card-label">Net revenue</span>
        </span>
        <span className="card-col card-end">
          <span className="card-label card-tabular">{money(week, data.currency)}</span>
          <Change now={week} before={before} />
        </span>
      </span>
      <Spark points={days} width={200} height={22} color="#30d158" fill />
    </Card>
  );
}

/** Total Revenue: every connected store's 30 days, in one currency, with the month's goal. */
export function TotalRevenueCard() {
  const currency = useSettings((s) => s.settings?.cards.revenueCurrency ?? "USD");
  const goal = useSettings((s) => s.settings?.cards.revenueGoal ?? 0);
  const { data, error } = useAsk<number>(`total/${currency}`, async () => {
    const all = await ipc.serviceRevenueAll();
    if (all.length === 0) throw "Add a store's key to a revenue card first";
    let sum = 0;
    for (const r of all) {
      const rate = r.currency === currency ? 1 : await ipc.fxRate(r.currency, currency);
      sum += r.total * rate;
    }
    return sum;
  });
  const percent = goal > 0 && data != null ? Math.min(100, (data / goal) * 100) : 0;
  return (
    <Card>
      <Ring value={percent} size={46} color="#30d158">
        <span className="card-small">{goal > 0 ? `${Math.round(percent)}%` : "30d"}</span>
      </Ring>
      <span className="card-col">
        <span className="card-label card-dim">Total revenue</span>
        <span className="card-big card-tabular">{data != null ? money(data, currency) : "—"}</span>
        <span className="card-faint card-small card-line">
          {error ? error : goal > 0 ? `goal ${money(goal, currency)}` : "last 30 days"}
        </span>
      </span>
    </Card>
  );
}

/** Plausible, DataFast, Dub: this week's visitors (or clicks), the trend and a line. */
export function TrafficCard({ service, unit }: { service: Service; unit: string }) {
  const { data, error } = useAsk<Traffic>(`traffic/${service}`, () => ipc.serviceTraffic(service));
  if (!data) return <Waiting service={service} error={error} />;
  return (
    <Card className="card-wide">
      <Mark service={service} />
      <span className="card-col">
        <span className="card-label card-dim">{unit} this week</span>
        <span className="card-row">
          <span className="card-big card-tabular">{compact(data.week)}</span>
          <Change now={data.week} before={data.previous} />
        </span>
      </span>
      <Spark
        points={data.daily}
        width={90}
        height={26}
        color={MARKS[service].color === "#111111" ? "#64d2ff" : MARKS[service].color}
        fill
      />
    </Card>
  );
}

/** Resend, Linear, Supabase, YouTube, Instagram, X, DeepSeek: a headline number and a note. */
export function CountCard({ service }: { service: Service }) {
  const { data, error } = useAsk<Count>(`count/${service}`, () => ipc.serviceCount(service));
  if (!data) return <Waiting service={service} error={error} />;
  const lines: Record<string, [string, string]> = {
    resend: ["emails · 7 days", `${data.second ?? 0} delivered`],
    linear: ["issues assigned", `${data.second ?? 0} in progress`],
    supabase: ["requests today", data.note ?? ""],
    youtube: ["subscribers", data.note ?? ""],
    instagram: ["followers", data.note ? `@${data.note}` : ""],
    x: ["followers", data.note ?? ""],
    deepseek: ["balance", data.note ?? ""],
  };
  const [label, note] = lines[service] ?? ["", ""];
  const value =
    service === "deepseek" ? money(data.value, data.note === "CNY" ? "CNY" : "USD") : compact(data.value);
  return (
    <Card>
      <Mark service={service} />
      <span className="card-col">
        <span className="card-label card-dim">{label}</span>
        <span className="card-big card-tabular">{value}</span>
        {note && (
          <span
            className={`card-faint card-small ${service === "supabase" ? (note === "Healthy" ? "card-green" : "card-red") : ""}`}
          >
            {note}
          </span>
        )}
      </span>
    </Card>
  );
}

/** Gmail: unread mail in the inbox. */
export function GmailCard() {
  const { data, error } = useAsk<number>("gmail", () => ipc.gmailUnread(), 300_000);
  return (
    <Card>
      <IconTile color="#ea4335">
        <Mail strokeWidth={2.2} />
      </IconTile>
      <span className="card-col card-text">
        <span className="card-label card-dim">Gmail</span>
        {data != null ? (
          <span className="card-big card-tabular">{data} unread</span>
        ) : (
          <span className="card-faint card-line">{error ? "Click to set up" : "Loading…"}</span>
        )}
      </span>
    </Card>
  );
}

/** GitHub: unread notifications. */
export function GithubNotificationsCard() {
  const { data, error } = useAsk("github", () => ipc.githubOverview(), 300_000);
  return (
    <Card>
      <IconTile color="#24292f">
        <Bell strokeWidth={2.2} />
      </IconTile>
      <span className="card-col card-text">
        <span className="card-label card-dim">GitHub</span>
        {data ? (
          <span className="card-big card-tabular">{data.notifications}</span>
        ) : (
          <span className="card-faint card-line">{error ? "Click to set up" : "Loading…"}</span>
        )}
      </span>
    </Card>
  );
}

/** GitHub Overview: notifications, open pull requests and stars. */
export function GithubOverviewCard() {
  const { data, error } = useAsk("github", () => ipc.githubOverview(), 300_000);
  if (!data) {
    return (
      <Card>
        <IconTile color="#24292f">
          <Bell strokeWidth={2.2} />
        </IconTile>
        <span className="card-faint">{error ? "Click to set up" : "Loading…"}</span>
      </Card>
    );
  }
  const stat = (icon: ReactNode, value: number, label: string) => (
    <span className="card-col card-center">
      <span className="card-row">
        {icon}
        <span className="card-label card-tabular">{compact(value)}</span>
      </span>
      <span className="card-faint card-small">{label}</span>
    </span>
  );
  return (
    <Card className="card-gap">
      {stat(<Bell className="card-blue" />, data.notifications, "unread")}
      {stat(<GitPullRequest className="card-green" />, data.pullRequests, "open PRs")}
      {stat(<Star className="card-yellow" />, data.stars, "stars")}
    </Card>
  );
}
