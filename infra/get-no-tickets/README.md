# get-no-tickets

Cloudflare Worker behind `https://get.no-tickets.com`. Proxies the
latest `no-tickets-installer.sh` from GitHub Releases so the documented
install command works:

```bash
curl -fsSL https://get.no-tickets.com | sh
```

## Deploy

The Worker was originally created via the Cloudflare dashboard.
Wrangler now manages it; subsequent changes ship through this directory.

```bash
# One-time
pnpm add -g wrangler           # or: brew install cloudflare-wrangler
wrangler login                 # browser-based OAuth

# Every change
cd infra/get-no-tickets
wrangler deploy
```

`wrangler deploy` is idempotent on the existing Worker as long as the
`name` in `wrangler.toml` matches the dashboard-created Worker. The
`custom_domain = true` route binding also reconciles cleanly with the
already-provisioned `get.no-tickets.com` custom domain.

## How it works

`src/index.js` returns the body of
`github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-installer.sh`
with `Content-Type: text/x-shellscript` and a 5-minute edge cache. On
upstream failure (rare — only when GitHub Releases is down) it returns
a shell script that prints a helpful error and exits 1.

## Tuning

- **Cache TTL**: `CACHE_MAX_AGE_SECONDS` in `src/index.js`. 5 min by
  default — lowest reasonable value before GitHub starts seeing
  per-edge-POP traffic; higher protects against GitHub flaps.
- **Domain**: bound via `routes` in `wrangler.toml`. To add a second
  hostname (e.g. `install.no-tickets.com`), add another entry with
  `custom_domain = true`.

## Test

After deploy:

```bash
curl -fsSL https://get.no-tickets.com | head -5   # shell-script preview
curl --proto '=https' --tlsv1.2 -LsSf https://get.no-tickets.com | sh   # real install
```

Local development (Worker runs on `localhost:8787`):

```bash
wrangler dev
curl http://localhost:8787
```
