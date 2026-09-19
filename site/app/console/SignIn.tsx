import "./console.css";

const ERRORS: Record<string, string> = {
  github_not_configured: "GitHub sign-in isn’t configured on this server yet.",
  oauth_state: "Sign-in expired or was tampered with. Try again.",
  oauth_token: "GitHub did not return an access token. Try again.",
  oauth_user: "Couldn’t read your GitHub profile. Try again.",
};

export default function SignIn({ error }: { error?: string }) {
  return (
    <div className="abc">
      <div className="signin">
        <div className="signin-card">
          <div className="signin-brand">
            <svg className="glyph" viewBox="0 0 32 32" fill="none" stroke="var(--ink)" strokeWidth="2.4" strokeLinecap="round">
              <circle cx="9" cy="7" r="3" />
              <circle cx="9" cy="25" r="3" />
              <circle cx="23" cy="16" r="3" />
              <path d="M9 10v12M9 16h4a6 6 0 0 0 6-6" />
            </svg>
            <span className="wm">snapshot<em>db</em></span>
          </div>

          <h1>Sign in to the console</h1>
          <p>
            Manage your sources and branches. You’ll connect the console to your self-hosted
            snapshotdb server with its access token after signing in.
          </p>

          {error && <div className="err">{ERRORS[error] || "Something went wrong. Try again."}</div>}

          <a className="btn primary gh" href="/api/auth/github">
            <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
              <path d="M8 .2a8 8 0 0 0-2.5 15.6c.4.1.5-.2.5-.4v-1.4c-2 .4-2.5-.5-2.7-1 0-.1-.5-1-.8-1.2-.3-.1-.7-.5 0-.5.6 0 1 .6 1.2.8.7 1.2 1.9.9 2.3.7.1-.5.3-.9.5-1.1-1.8-.2-3.6-.9-3.6-4 0-.9.3-1.6.8-2.1 0-.2-.3-1 .1-2.1 0 0 .7-.2 2.2.8a7.5 7.5 0 0 1 4 0c1.5-1 2.2-.8 2.2-.8.4 1.1.1 1.9.1 2.1.5.5.8 1.2.8 2.1 0 3.1-1.9 3.8-3.6 4 .3.2.5.7.5 1.4v2.1c0 .2.1.5.5.4A8 8 0 0 0 8 .2Z" />
            </svg>
            Continue with GitHub
          </a>
        </div>
      </div>
    </div>
  );
}
