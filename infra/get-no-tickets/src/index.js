// get-no-tickets — Cloudflare Worker serving the install scripts at
// https://get.no-tickets.com so users can run:
//
//   curl -fsSL https://get.no-tickets.com | sh                            (POSIX)
//   irm https://get.no-tickets.com/installer.ps1 | iex                    (Windows)
//
// Backed by the `no-tickets-installer.{sh,ps1}` artifacts that cargo-dist
// publishes to the latest GitHub Release on every tagged release.

const SHELL_INSTALLER_URL =
  "https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-installer.sh";
const POWERSHELL_INSTALLER_URL =
  "https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-installer.ps1";

// 5-minute edge cache — new releases are visible within 5 min of being
// published. Lower to ~30 s for near-instant pickup; raise for protection
// against transient GitHub Release flaps.
const CACHE_MAX_AGE_SECONDS = 300;

function pickUpstream(pathname) {
  // `/installer.ps1` (case-insensitive) routes to the PowerShell installer.
  // Everything else (`/`, `/installer.sh`, anything unknown) serves the
  // POSIX shell installer — the dominant use case.
  if (pathname.toLowerCase().endsWith("/installer.ps1")) {
    return {
      url: POWERSHELL_INSTALLER_URL,
      contentType: "text/plain; charset=utf-8",
      errorFallback: `# get.no-tickets.com: failed to fetch installer\n# Try the direct URL: ${POWERSHELL_INSTALLER_URL}\nexit 1\n`,
    };
  }
  return {
    url: SHELL_INSTALLER_URL,
    contentType: "text/x-shellscript",
    errorFallback: `# get.no-tickets.com: failed to fetch installer\n# Try the direct URL: ${SHELL_INSTALLER_URL}\nexit 1\n`,
  };
}

export default {
  async fetch(request) {
    const url = new URL(request.url);
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
