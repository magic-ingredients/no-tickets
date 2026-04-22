import { describe, it, expect, afterEach } from 'vitest';
import { startAuthServer } from '../auth-server.js';

describe('startAuthServer', () => {
  let cleanup: (() => Promise<void>) | undefined;
  let pendingToken: Promise<string> | undefined;

  afterEach(async () => {
    if (cleanup) {
      await cleanup();
      cleanup = undefined;
    }
    if (pendingToken) {
      await pendingToken.catch(() => {});
      pendingToken = undefined;
    }
  });

  it('starts an HTTP server on a random available port', async () => {
    const { port, tokenPromise, close } = await startAuthServer();
    cleanup = close;
    pendingToken = tokenPromise;

    expect(port).toBeGreaterThan(0);
    expect(port).toBeLessThanOrEqual(65535);
  });

  it('resolves with the token when callback receives a token query param', async () => {
    const { port, tokenPromise, close } = await startAuthServer();
    cleanup = close;

    const response = await fetch(`http://127.0.0.1:${port}/callback?token=nt_session_test123`);

    expect(response.status).toBe(200);
    expect(response.headers.get('content-type')).toContain('text/plain');
    const body = await response.text();
    expect(body).toBe('Authentication successful. You can close this tab.');

    const token = await tokenPromise;
    expect(token).toBe('nt_session_test123');
  });

  it('returns 400 when token param is empty', async () => {
    const { port, tokenPromise, close } = await startAuthServer();
    cleanup = close;
    pendingToken = tokenPromise;

    const response = await fetch(`http://127.0.0.1:${port}/callback?token=`);

    expect(response.status).toBe(400);
  });

  it('returns 400 when callback is missing the token param', async () => {
    const { port, tokenPromise, close } = await startAuthServer();
    cleanup = close;
    pendingToken = tokenPromise;

    const response = await fetch(`http://127.0.0.1:${port}/callback`);

    expect(response.status).toBe(400);
  });

  it('returns 404 for non-callback paths', async () => {
    const { port, tokenPromise, close } = await startAuthServer();
    cleanup = close;
    pendingToken = tokenPromise;

    const response = await fetch(`http://127.0.0.1:${port}/other`);

    expect(response.status).toBe(404);
  });

  it('shuts down the server after receiving a valid token', async () => {
    const { port, tokenPromise, close } = await startAuthServer();
    cleanup = close;

    await fetch(`http://127.0.0.1:${port}/callback?token=nt_session_abc`);
    await tokenPromise;

    // Server should be closed — connection should fail
    await expect(
      fetch(`http://127.0.0.1:${port}/callback?token=another`)
    ).rejects.toThrow();
  });

  it('rejects the token promise on timeout', async () => {
    const { tokenPromise, close } = await startAuthServer({ timeoutMs: 50 });
    cleanup = close;

    await expect(tokenPromise).rejects.toThrow('timed out');
  });

  it('rejects the token promise when close() is called before token arrives', async () => {
    const { tokenPromise, close } = await startAuthServer();

    await close();

    await expect(tokenPromise).rejects.toThrow('Auth server closed');
  });

  it('uses only the first token when callback is called twice', async () => {
    const { port, tokenPromise, close } = await startAuthServer();
    cleanup = close;

    await fetch(`http://127.0.0.1:${port}/callback?token=first_token`);
    const token = await tokenPromise;

    expect(token).toBe('first_token');
  });

  it('does not reject after successful token even if timeout is short', async () => {
    const { port, tokenPromise, close } = await startAuthServer({ timeoutMs: 100 });
    cleanup = close;

    await fetch(`http://127.0.0.1:${port}/callback?token=nt_session_fast`);
    const token = await tokenPromise;

    // Re-await the already-resolved promise to confirm it stays resolved
    const tokenAgain = await tokenPromise;
    expect(token).toBe('nt_session_fast');
    expect(tokenAgain).toBe('nt_session_fast');
  });

  it('close() after token received does not change the resolved value', async () => {
    const { port, tokenPromise, close } = await startAuthServer();

    await fetch(`http://127.0.0.1:${port}/callback?token=nt_session_keep`);
    const token = await tokenPromise;

    await close();

    // tokenPromise should still be resolved with same value
    const tokenAgain = await tokenPromise;
    expect(tokenAgain).toBe(token);
  });

  it('close() can be called safely even after server already shut down', async () => {
    const { port, tokenPromise, close } = await startAuthServer();

    await fetch(`http://127.0.0.1:${port}/callback?token=nt_session_abc`);
    await tokenPromise;

    // Should not throw
    await close();
    await close();
  });
});
