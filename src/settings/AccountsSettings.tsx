import { useEffect, useState, type ReactNode } from "react";
import { Check, Copy } from "lucide-react";

import type { KeyAccount } from "../core/generated/KeyAccount";
import type { OAuthProvider } from "../core/generated/OAuthProvider";
import type { Settings } from "../core/generated/Settings";
import { errorText, featuresIpc } from "../core/ipcFeatures";
import { useAccounts, watchAccounts } from "../widgets/accounts/status";

type Save = (patch: Partial<Settings>) => void;

/** A key pasted in, sent straight to Windows Credential Manager. Never shown again. */
function KeyField({ account, saved, label }: { account: KeyAccount; saved: boolean; label: string }) {
  const [value, setValue] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const store = (text: string) => {
    setBusy(true);
    featuresIpc
      .accountKeySet(account, text)
      .then((status) => {
        useAccounts.getState().set(status);
        setValue("");
        setError(null);
      })
      .catch((e) => setError(errorText(e, "The key couldn't be saved.")))
      .finally(() => setBusy(false));
  };
  return (
    <div className="account-field">
      <span className="account-label">{label}</span>
      <span className="account-inputs">
        <input
          className="settings-input"
          type="password"
          autoComplete="off"
          spellCheck={false}
          placeholder={saved ? "Saved in Credential Manager" : "Paste it here"}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          aria-label={label}
        />
        <button className="settings-quit" disabled={busy || !value.trim()} onClick={() => store(value)}>
          Save
        </button>
        {saved && (
          <button className="settings-link" disabled={busy} onClick={() => store("")}>
            Remove
          </button>
        )}
      </span>
      {error && (
        <span className="settings-error" role="alert">
          {error}
        </span>
      )}
    </div>
  );
}

/** A setting typed in (not a secret), saved when the box loses focus. */
function TextField({
  label,
  value,
  placeholder,
  onSave,
  multiline,
}: {
  label: string;
  value: string;
  placeholder?: string;
  onSave: (value: string) => void;
  multiline?: boolean;
}) {
  // Remounted (by key) when the saved value changes elsewhere.
  const [text, setText] = useState(value);
  const commit = () => text !== value && onSave(text);
  return (
    <label className="account-field">
      <span className="account-label">{label}</span>
      {multiline ? (
        <textarea
          className="settings-input account-area"
          value={text}
          placeholder={placeholder}
          rows={3}
          spellCheck={false}
          onChange={(e) => setText(e.target.value)}
          onBlur={commit}
        />
      ) : (
        <input
          className="settings-input"
          value={text}
          placeholder={placeholder}
          spellCheck={false}
          onChange={(e) => setText(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => e.key === "Enter" && commit()}
        />
      )}
    </label>
  );
}

function SignIn({
  provider,
  connected,
  ready,
}: {
  provider: OAuthProvider;
  connected: boolean;
  ready: boolean;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = (work: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    work()
      .then(() => useAccounts.getState().load())
      .catch((e) => setError(errorText(e, "Signing in didn't work.")))
      .finally(() => setBusy(false));
  };
  return (
    <div className="account-field">
      <span className="account-inputs">
        {connected ? (
          <>
            <span className="account-ok">
              <Check size={14} /> Signed in
            </span>
            <button
              className="settings-link"
              onClick={() => run(() => featuresIpc.accountDisconnect(provider))}
            >
              Sign out
            </button>
          </>
        ) : (
          <button
            className="settings-quit"
            disabled={busy || !ready}
            onClick={() => run(() => featuresIpc.accountConnect(provider))}
          >
            {busy ? "Finish in your browser…" : "Sign in"}
          </button>
        )}
      </span>
      {error && (
        <span className="settings-error" role="alert">
          {error}
        </span>
      )}
    </div>
  );
}

function Account({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  return (
    <div className="account-card" data-setting={id}>
      <h3>{title}</h3>
      {children}
    </div>
  );
}

function CopyText({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <span className="account-copy">
      <code>{text}</code>
      <button
        className="widget-move"
        aria-label="Copy"
        title="Copy"
        onClick={() =>
          navigator.clipboard
            .writeText(text)
            .then(() => setCopied(true))
            .catch(() => setCopied(false))
        }
      >
        {copied ? <Check size={14} /> : <Copy size={14} />}
      </button>
    </span>
  );
}

/** Keys, tokens and sign-ins for the account widgets. */
export function AccountsSection({ settings, save }: { settings: Settings; save: Save }) {
  const status = useAccounts((s) => s.status);
  useEffect(() => watchAccounts(), []);
  const accounts = settings.accounts;
  const saveAccounts = (patch: Partial<Settings["accounts"]>) =>
    save({ accounts: { ...accounts, ...patch } });
  if (!status) return null;

  return (
    <section>
      <h2>Accounts</h2>
      <p className="settings-hint">
        For the widgets that use your own accounts. Keys and tokens go straight to Windows Credential Manager;
        they're never written to Dock's settings file or shown again.
      </p>

      <Account id="anthropic" title="Ask Claude">
        <p className="settings-hint">
          Create an API key at console.anthropic.com → API keys. Questions go to Claude Sonnet 5.5 and are
          billed to that key.
        </p>
        <KeyField account="anthropic" saved={status.anthropic} label="Anthropic API key" />
      </Account>

      <Account id="spotify" title="Spotify playlists">
        <p className="settings-hint">
          Create an app at developer.spotify.com/dashboard, tick "Web API", and add this redirect URI to it.
          Then paste its client ID here and sign in. Starting music needs Spotify Premium.
        </p>
        <CopyText text={status.spotifyRedirect} />
        <TextField
          key={accounts.spotifyClientId}
          label="Client ID"
          value={accounts.spotifyClientId}
          placeholder="From your Spotify app's settings"
          onSave={(spotifyClientId) => saveAccounts({ spotifyClientId })}
        />
        <SignIn provider="spotify" connected={status.spotify} ready={accounts.spotifyClientId !== ""} />
      </Account>

      <Account id="notion" title="Notion tasks">
        <p className="settings-hint">
          Create an internal integration at notion.so/profile/integrations and paste its token. Then open your
          tasks database, choose ••• → Connections and add the integration, and paste the database's link.
          Tasks are ticked with its Done checkbox, or its status' Complete option.
        </p>
        <KeyField account="notion" saved={status.notion} label="Integration token" />
        <TextField
          key={accounts.notionDatabaseId}
          label="Database link or ID"
          value={accounts.notionDatabaseId}
          placeholder="https://www.notion.so/…"
          onSave={(notionDatabaseId) => saveAccounts({ notionDatabaseId })}
        />
      </Account>

      <Account id="google" title="Google Tasks">
        <p className="settings-hint">
          In Google Cloud Console, turn on the Google Tasks API, then create an OAuth client of type "Desktop
          app". Paste its client ID and client secret (Google needs it even for desktop apps), and sign in.
        </p>
        <TextField
          key={accounts.googleClientId}
          label="Client ID"
          value={accounts.googleClientId}
          placeholder="….apps.googleusercontent.com"
          onSave={(googleClientId) => saveAccounts({ googleClientId })}
        />
        <KeyField account="googleSecret" saved={status.googleSecret} label="Client secret" />
        <SignIn provider="google" connected={status.google} ready={accounts.googleClientId !== ""} />
      </Account>

      <Account id="github" title="GitHub Actions">
        <p className="settings-hint">
          Create a fine-grained token at github.com/settings/tokens with read access to "Actions" for your
          repos (a classic token with the repo scope works too). List repos one per line as owner/name.
        </p>
        <KeyField account="github" saved={status.github} label="Personal access token" />
        <TextField
          key={accounts.githubRepos.join("\n")}
          label="Repos"
          multiline
          value={accounts.githubRepos.join("\n")}
          placeholder={"owner/repo\nowner/another"}
          onSave={(text) =>
            saveAccounts({
              githubRepos: text
                .split(/[\n,]+/)
                .map((r) => r.trim())
                .filter(Boolean),
            })
          }
        />
      </Account>
    </section>
  );
}
