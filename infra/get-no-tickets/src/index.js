// get-no-tickets — Cloudflare Worker serving the install.sh content at
// https://get.no-tickets.com so users can run:
//
//   curl -fsSL https://get.no-tickets.com | sh
//
// Backed by the `no-tickets-installer.sh` artifact that cargo-dist
// publishes to the latest GitHub Release on every tagged release.

const INSTALLER_URL =
  "https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-installer.sh";

// 5-minute edge cache — new releases are visible within 5 min of being
// published. Lower to ~30 s for near-instant pickup; raise for protection
// against transient GitHub Release flaps.
const CACHE_MAX_AGE_SECONDS = 300;

export default {
  async fetch() {
    const upstream = await fetch(INSTALLER_URL, { redirect: "follow" });
    if (!upstream.ok) {
      return new Response(
        `# get.no-tickets.com: failed to fetch installer from upstream (HTTP ${upstream.status})\n` +
          `# Try the direct URL: ${INSTALLER_URL}\n` +
          `exit 1\n`,
        {
          status: 502,
          headers: { "Content-Type": "text/x-shellscript" },
        },
      );
    }
    return new Response(upstream.body, {
      status: 200,
      headers: {
        "Content-Type": "text/x-shellscript",
        "Cache-Control": `public, max-age=${CACHE_MAX_AGE_SECONDS}`,
      },
    });
  },
};
