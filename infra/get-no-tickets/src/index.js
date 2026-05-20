// get-no-tickets — Cloudflare Worker serving the install scripts at
// https://get.no-tickets.com so users can run:
//
//   curl -fsSL https://get.no-tickets.com | sh                            (POSIX)
//   irm https://get.no-tickets.com/installer.ps1 | iex                    (Windows)
//
// Backed by the `no-tickets-installer.{sh,ps1}` artifacts that cargo-dist
// publishes to the latest GitHub Release on every tagged release.
//
// Path routing (`pickUpstream`):
//   /                      → POSIX shell installer (default for `curl … | sh`)
//   /installer.sh          → POSIX shell installer (explicit)
//   /install.sh            → POSIX shell installer (rustup / deno / bun convention alias)
//   /installer.ps1         → PowerShell installer
//
// Content negotiation:
//   GET /                  with `Accept: text/html` → HTML landing page
//                          (browser visitors don't see a wall of shell)
//   Everything else        → installer body with the right shell content-type

const SHELL_INSTALLER_URL =
  "https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-installer.sh";
const POWERSHELL_INSTALLER_URL =
  "https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-installer.ps1";

// 5-minute edge cache — new releases are visible within 5 min of being
// published. Lower to ~30 s for near-instant pickup; raise for protection
// against transient GitHub Release flaps.
const CACHE_MAX_AGE_SECONDS = 300;

function pickUpstream(pathname) {
  const p = pathname.toLowerCase();
  // PowerShell installer routes
  if (p.endsWith("/installer.ps1")) {
    return {
      url: POWERSHELL_INSTALLER_URL,
      contentType: "text/plain; charset=utf-8",
      errorFallback: `# get.no-tickets.com: failed to fetch installer\n# Try the direct URL: ${POWERSHELL_INSTALLER_URL}\nexit 1\n`,
    };
  }
  // POSIX shell installer — accept both cargo-dist's `installer.sh`
  // and the `install.sh` convention used by rustup / deno / bun /
  // most pipe-to-shell installers. Defaulting (`/`) also lands here.
  return {
    url: SHELL_INSTALLER_URL,
    contentType: "text/x-shellscript",
    errorFallback: `# get.no-tickets.com: failed to fetch installer\n# Try the direct URL: ${SHELL_INSTALLER_URL}\nexit 1\n`,
  };
}

// Tiny HTML landing for browser visitors at `/`. Self-contained (no
// external JS / CSS / fonts) so it loads instantly and has no
// supply-chain surface.
const LANDING_HTML = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>no-tickets — install</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>
  body { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; max-width: 42rem; margin: 4rem auto; padding: 0 1.5rem; color: #1a1a1a; line-height: 1.5; }
  h1 { font-weight: 600; margin-bottom: 0.25rem; }
  p.tagline { color: #555; margin-top: 0; }
  pre { background: #f4f4f4; padding: 1rem; border-radius: 6px; overflow-x: auto; }
  code { font: inherit; }
  a { color: #0366d6; }
  .label { color: #666; font-size: 0.85rem; margin: 1.5rem 0 0.25rem; text-transform: uppercase; letter-spacing: 0.05em; }
</style>
</head>
<body>
<h1>no-tickets</h1>
<p class="tagline">Ticketless project management for AI teams.</p>

<div class="label">macOS / Linux</div>
<pre><code>curl -fsSL https://get.no-tickets.com | sh</code></pre>

<div class="label">Homebrew</div>
<pre><code>brew install magic-ingredients/tap/no-tickets</code></pre>

<div class="label">Windows (PowerShell)</div>
<pre><code>powershell -ExecutionPolicy ByPass -c "irm https://get.no-tickets.com/installer.ps1 | iex"</code></pre>

<p>Full install matrix and CI recipes:
<a href="https://github.com/magic-ingredients/no-tickets/blob/main/docs/install.md">docs/install.md</a>.</p>

<p>Source: <a href="https://github.com/magic-ingredients/no-tickets">github.com/magic-ingredients/no-tickets</a></p>
</body>
</html>
`;

function wantsHtml(request, pathname) {
  // Only intercept the bare `/` path — explicit shell-installer paths
  // (`/installer.sh`, `/install.sh`) MUST always serve the script
  // regardless of Accept header, otherwise `curl -H 'Accept: …' | sh`
  // pipelines would break.
  if (pathname !== "/" && pathname !== "") {
    return false;
  }
  const accept = request.headers.get("Accept") || "";
  // Browsers send `text/html` first in their Accept; curl's default is
  // `*/*`. Match `text/html` explicitly to avoid serving HTML to
  // anything-goes consumers.
  return accept.includes("text/html");
}

export default {
  async fetch(request) {
    const url = new URL(request.url);

    if (wantsHtml(request, url.pathname)) {
      return new Response(LANDING_HTML, {
        status: 200,
        headers: {
          "Content-Type": "text/html; charset=utf-8",
          "Cache-Control": `public, max-age=${CACHE_MAX_AGE_SECONDS}`,
        },
      });
    }

    const target = pickUpstream(url.pathname);

    const upstream = await fetch(target.url, { redirect: "follow" });
    if (!upstream.ok) {
      return new Response(
        target.errorFallback.replace(
          "failed to fetch installer",
          `failed to fetch installer from upstream (HTTP ${upstream.status})`,
        ),
        {
          status: 502,
          headers: { "Content-Type": target.contentType },
        },
      );
    }
    return new Response(upstream.body, {
      status: 200,
      headers: {
        "Content-Type": target.contentType,
        "Cache-Control": `public, max-age=${CACHE_MAX_AGE_SECONDS}`,
      },
    });
  },
};
