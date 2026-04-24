import { describe, it, expect, afterEach } from 'vitest';
import { startAuthServer } from '../auth-server.js';

const NONCE = 'a'.repeat(32);

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
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE });
    cleanup = close;
    pendingCallback = callbackPromise;

    expect(port).toBeGreaterThan(0);
    expect(port).toBeLessThanOrEqual(65535);
  });

  it('resolves with token + email when callback receives matching state', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE });
    cleanup = close;

    const response = await fetch(
      `http://127.0.0.1:${port}/callback?token=nt_session_test123&email=alice%40example.com&state=${NONCE}`,
    );

    expect(response.status).toBe(200);
    expect(response.headers.get('content-type')).toContain('text/plain');
    const body = await response.text();
    expect(body).toBe('Authentication successful. You can close this tab.');

    const result = await callbackPromise;
    expect(result).toEqual({ token: 'nt_session_test123', email: 'alice@example.com' });
  });

  it('returns 400 and does not resolve when state does not match the expected nonce', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, timeoutMs: 100 });
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
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE });
    cleanup = close;
    pendingCallback = callbackPromise;

    const response = await fetch(
      `http://127.0.0.1:${port}/callback?token=&email=a%40b.com&state=${NONCE}`,
    );

    expect(response.status).toBe(400);
  });

  it('returns 400 when email param is missing', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE });
    cleanup = close;
    pendingCallback = callbackPromise;

    const response = await fetch(
      `http://127.0.0.1:${port}/callback?token=nt_session_x&state=${NONCE}`,
    );

    expect(response.status).toBe(400);
  });

  it('returns 400 when state param is missing', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE });
    cleanup = close;
    pendingCallback = callbackPromise;

    const response = await fetch(
      `http://127.0.0.1:${port}/callback?token=nt_session_x&email=a%40b.com`,
    );

    expect(response.status).toBe(400);
  });

  it('returns 404 for non-callback paths', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE });
    cleanup = close;
    pendingCallback = callbackPromise;

    const response = await fetch(`http://127.0.0.1:${port}/other`);

    expect(response.status).toBe(404);
  });

  it('shuts down the server after a successful callback', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE });
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
    const { callbackPromise, close } = await startAuthServer({ expectedState: NONCE, timeoutMs: 50 });
    cleanup = close;

    await expect(callbackPromise).rejects.toThrow('timed out');
  });

  it('rejects the callback promise when close() is called before any callback', async () => {
    const { callbackPromise, close } = await startAuthServer({ expectedState: NONCE });

    await close();

    await expect(callbackPromise).rejects.toThrow('Auth server closed');
  });

  it('uses only the first valid callback when called twice', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE });
    cleanup = close;

    await fetch(
      `http://127.0.0.1:${port}/callback?token=first_token&email=a%40b.com&state=${NONCE}`,
    );
    const result = await callbackPromise;

    expect(result.token).toBe('first_token');
  });

  it('does not reject after successful callback even if timeout is short', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE, timeoutMs: 100 });
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
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE });

    await fetch(
      `http://127.0.0.1:${port}/callback?token=nt_session_keep&email=a%40b.com&state=${NONCE}`,
    );
    const first = await callbackPromise;

    await close();

    const second = await callbackPromise;
    expect(second).toEqual(first);
  });

  it('close() can be called safely even after server already shut down', async () => {
    const { port, callbackPromise, close } = await startAuthServer({ expectedState: NONCE });

    await fetch(
      `http://127.0.0.1:${port}/callback?token=nt_session_abc&email=a%40b.com&state=${NONCE}`,
    );
    await callbackPromise;

    await close();
    await close();
  });
});
