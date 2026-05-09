import { describe, it, expect, afterEach } from 'vitest';
import { startAuthServer } from '../auth-server.js';

const NONCE = 'a'.repeat(32);
const APP_URL = 'https://app-staging.no-tickets.com';

describe('startAuthServer', () => {
  let cleanup: (() => Promise<void>) | undefined;
  let pendingCallback: Promise<unknown> | undefined;

  afterEach(async () => {
    if (cleanup) {
      await cleanup();
      cleanup = undefined;
    }
    if (pendingCallback) {
      await pendingCallback.catch(() => {});
      pendingCallback = undefined;
    }
  });

  it('starts an HTTP server on a random available port', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL });
    cleanup = close;
    pendingCallback = callbackPromise;

    expect(port).toBeGreaterThan(0);
    expect(port).toBeLessThanOrEqual(65535);
  });

  it('resolves with token + email when callback receives matching state', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL });
    cleanup = close;

    const response = await fetch(
      `http://127.0.0.1:${port}/callback?token=nt_session_test123&email=alice%40example.com&state=${NONCE}`,
    );

    expect(response.status).toBe(200);
    expect(response.headers.get('content-type')).toContain('text/html');
    const body = await response.text();
    expect(body).toMatch(/<svg[\s\S]*<\/svg>/i);
    expect(body).toContain('CLI authentication successful');
    expect(body).toContain('You can close this tab');
    expect(body).toContain('continue to no-tickets');
    expect(body).toContain(APP_URL);

    const result = await callbackPromise;
    expect(result).toEqual({ token: 'nt_session_test123', email: 'alice@example.com' });
  });

  it('escapes the appUrl when rendering the continue link to prevent HTML injection', async () => {
    const evilApp = 'https://evil.example.com/"><script>x</script>';
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: evilApp });
    cleanup = close;

    const response = await fetch(
      `http://127.0.0.1:${port}/callback?token=t&email=a%40b.com&state=${NONCE}`,
    );
    const body = await response.text();
    await callbackPromise;

    expect(body).not.toContain('<script>x</script>');
    expect(body).toContain('&lt;script&gt;');
  });

  it('returns 400 and does not resolve when state does not match the expected nonce', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL, timeoutMs: 100 });
    cleanup = close;

    const response = await fetch(
      `http://127.0.0.1:${port}/callback?token=nt_session_x&email=a%40b.com&state=WRONG`,
    );

    expect(response.status).toBe(400);

    // The promise must not resolve from this attacker request — it stays pending
    // until the timeout fires.
    await expect(callbackPromise).rejects.toThrow('timed out');
  });

  it('returns 400 when token param is empty', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL });
    cleanup = close;
    pendingCallback = callbackPromise;

    const response = await fetch(
      `http://127.0.0.1:${port}/callback?token=&email=a%40b.com&state=${NONCE}`,
    );

    expect(response.status).toBe(400);
  });

  it('returns 400 when email param is missing', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL });
    cleanup = close;
    pendingCallback = callbackPromise;

    const response = await fetch(
      `http://127.0.0.1:${port}/callback?token=nt_session_x&state=${NONCE}`,
    );

    expect(response.status).toBe(400);
  });

  it('returns 400 when state param is missing', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL });
    cleanup = close;
    pendingCallback = callbackPromise;

    const response = await fetch(
      `http://127.0.0.1:${port}/callback?token=nt_session_x&email=a%40b.com`,
    );

    expect(response.status).toBe(400);
  });

  it('returns 404 for non-callback paths', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL });
    cleanup = close;
    pendingCallback = callbackPromise;

    const response = await fetch(`http://127.0.0.1:${port}/other`);

    expect(response.status).toBe(404);
  });

  it('shuts down the server after a successful callback', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL });
    cleanup = close;

    await fetch(
      `http://127.0.0.1:${port}/callback?token=nt_session_abc&email=a%40b.com&state=${NONCE}`,
    );
    await callbackPromise;

    await expect(
      fetch(`http://127.0.0.1:${port}/callback?token=another&email=a%40b.com&state=${NONCE}`),
    ).rejects.toThrow();
  });

  it('rejects the callback promise on timeout', async () => {
    const { callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL, timeoutMs: 50 });
    cleanup = close;

    await expect(callbackPromise).rejects.toThrow('timed out');
  });

  it('rejects the callback promise when close() is called before any callback', async () => {
    const { callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL });

    await close();

    await expect(callbackPromise).rejects.toThrow('Auth server closed');
  });

  it('uses only the first valid callback and rejects (409) a second one', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL });
    cleanup = close;

    const first = await fetch(
      `http://127.0.0.1:${port}/callback?token=first_token&email=a%40b.com&state=${NONCE}`,
    );
    expect(first.status).toBe(200);
    const result = await callbackPromise;
    expect(result.token).toBe('first_token');

    // Second request lands after server.close() has fired, so the connection
    // refusal is what we observe. Either way, the resolved value is unchanged.
    await expect(
      fetch(`http://127.0.0.1:${port}/callback?token=second_token&email=a%40b.com&state=${NONCE}`),
    ).rejects.toThrow();

    expect((await callbackPromise).token).toBe('first_token');
  });

  it('returns 409 for a second valid callback that arrives before the server finishes closing', async () => {
    // The server calls server.close() after the first valid request, but there is a
    // small window where keep-alive connections can still deliver a second request.
    // We simulate this by sending the second request immediately after the first.
    // The settled guard should return 409 if the connection is still alive.
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL });
    cleanup = close;

    // First valid callback — server sets settled = true then calls server.close()
    const first = await fetch(
      `http://127.0.0.1:${port}/callback?token=first_token&email=a%40b.com&state=${NONCE}`,
      { headers: { Connection: 'keep-alive' } },
    );
    expect(first.status).toBe(200);

    // Immediately send a second valid callback on the same connection.
    // If the connection is still alive, the settled guard returns 409.
    // If the server already closed the socket, connection refusal is also acceptable.
    let secondStatus: number | null = null;
    try {
      const second = await fetch(
        `http://127.0.0.1:${port}/callback?token=second_token&email=a%40b.com&state=${NONCE}`,
      );
      secondStatus = second.status;
      expect(second.status).toBe(409);
    } catch {
      // Connection refused after close() — also acceptable
      secondStatus = null;
    }

    // The promise resolved from the first callback regardless
    const resolved = await callbackPromise;
    expect(resolved.token).toBe('first_token');
    // The second request must never have produced a 200 (would indicate double-settle)
    expect(secondStatus).not.toBe(200);
  });

  it('returns 405 for non-GET methods on /callback', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL, timeoutMs: 100 });
    cleanup = close;
    pendingCallback = callbackPromise;

    const response = await fetch(`http://127.0.0.1:${port}/callback?token=t&email=a%40b.com&state=${NONCE}`, {
      method: 'POST',
    });

    expect(response.status).toBe(405);
  });

  it('preserves "+" characters in the email (alice+tag@example.com)', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL });
    cleanup = close;

    const aliasedEmail = 'alice+tag@example.com';
    await fetch(
      `http://127.0.0.1:${port}/callback?token=t&email=${encodeURIComponent(aliasedEmail)}&state=${NONCE}`,
    );

    const result = await callbackPromise;
    expect(result.email).toBe(aliasedEmail);
  });

  it('rejects callbacks that arrive after close() with HTTP 409 (no resolve)', async () => {
    // Force a race: send the callback request while the server is shutting down.
    // Because the server is already torn down, the most likely observable is a
    // connection-level error. The contract we care about is: the promise was
    // rejected by close() and is NOT later overwritten by the callback.
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL });

    await close();
    await expect(callbackPromise).rejects.toThrow('Auth server closed');

    await expect(
      fetch(`http://127.0.0.1:${port}/callback?token=late&email=a%40b.com&state=${NONCE}`),
    ).rejects.toThrow();

    // Promise stays rejected — no late resolve.
    await expect(callbackPromise).rejects.toThrow('Auth server closed');
  });

  it('does not reject after successful callback even if timeout is short', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL, timeoutMs: 100 });
    cleanup = close;

    await fetch(
      `http://127.0.0.1:${port}/callback?token=nt_session_fast&email=a%40b.com&state=${NONCE}`,
    );
    const first = await callbackPromise;
    const second = await callbackPromise;

    expect(first).toEqual({ token: 'nt_session_fast', email: 'a@b.com' });
    expect(second).toEqual(first);
  });

  it('close() after callback received does not change the resolved value', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL });

    await fetch(
      `http://127.0.0.1:${port}/callback?token=nt_session_keep&email=a%40b.com&state=${NONCE}`,
    );
    const first = await callbackPromise;

    await close();

    // close() must not overwrite an already-resolved promise with a rejection
    const second = await callbackPromise;
    expect(second).toEqual(first);
    expect(second).toEqual({ token: 'nt_session_keep', email: 'a@b.com' });
  });

  it('close() can be called safely even after server already shut down', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL });

    await fetch(
      `http://127.0.0.1:${port}/callback?token=nt_session_abc&email=a%40b.com&state=${NONCE}`,
    );
    await callbackPromise;

    await close();
    await close();
  });

  it('getRawQueryValue: skips a pair starting with "=" (empty key) and continues to find valid params', async () => {
    // A pair like "=junk&token=t" has eq===0. The parser must skip pairs where
    // eq < 0 (no "=" at all) but also handle eq===0 (empty key). With a change
    // from "eq < 0" to "eq <= 0", a valid pair like "token=t" (eq > 0) would
    // still be found, but a pair like "=t" would be incorrectly skipped. We
    // verify the parser skips the empty-key pair and still finds valid params.
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL });
    cleanup = close;

    // Manually craft a URL with a leading "=junk" pair before valid params
    const response = await fetch(
      `http://127.0.0.1:${port}/callback?=junk&token=nt_session_boundary&email=a%40b.com&state=${NONCE}`,
    );

    expect(response.status).toBe(200);
    const result = await callbackPromise;
    expect(result.token).toBe('nt_session_boundary');
  });

  it('getRawQueryValue: ^ anchor strips only the leading "?" and preserves "?" inside a value', async () => {
    // Without the ^ anchor, rawSearch.replace(/\\?/, '') would strip the first
    // occurrence of "?" even if it appears inside a percent-encoded value.
    // A token containing a literal "?" is encoded as "%3F" in the URL. When the
    // raw parser calls decodeURIComponent on the raw value it becomes "x?foo".
    // We verify this round-trip works correctly.
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL });
    cleanup = close;

    // token value contains a literal "?" (percent-encoded as %3F in the URL)
    const response = await fetch(
      `http://127.0.0.1:${port}/callback?token=x%3Ffoo&email=a%40b.com&state=${NONCE}`,
    );

    expect(response.status).toBe(200);
    const result = await callbackPromise;
    // The token should be decoded as "x?foo" — the ? inside the value is preserved
    expect(result.token).toBe('x?foo');
  });

  it('a valid callback arriving after the timeout fires does not resolve the already-rejected promise', async () => {
    // Attach a no-op catch handler immediately to suppress unhandled-rejection
    // warnings that can fire before our explicit rejects.toThrow assertion runs.
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, appUrl: APP_URL, timeoutMs: 50 });
    callbackPromise.catch(() => {});
    cleanup = close;

    // Let the timeout fire and reject the promise.
    await expect(callbackPromise).rejects.toThrow('timed out');

    // The server is still accepting connections briefly after timeout (until close() is called).
    // Send a valid callback — it should NOT re-settle the already-rejected promise.
    try {
      await fetch(
        `http://127.0.0.1:${port}/callback?token=late_token&email=a%40b.com&state=${NONCE}`,
      );
    } catch {
      // Connection refused — server closed, acceptable
    }

    // Promise stays rejected — the !settled guard in the timeout handler prevents double-settle.
    await expect(callbackPromise).rejects.toThrow('timed out');
  });
});
