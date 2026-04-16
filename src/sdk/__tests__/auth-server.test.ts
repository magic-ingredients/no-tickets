import { describe, it, expect, afterEach } from 'vitest';
import { startAuthServer } from '../auth-server.js';

describe('startAuthServer', () => {
  let cleanup: (() => Promise<void>) | undefined;

  afterEach(async () => {
    if (cleanup) {
      await cleanup();
      cleanup = undefined;
    }
  });

  it('starts an HTTP server on a random available port', async () => {
    const { port, close } = await startAuthServer();
    cleanup = close;

    expect(port).toBeGreaterThan(0);
    expect(port).toBeLessThanOrEqual(65535);
  });

  it('resolves with the token when callback receives a token query param', async () => {
    const { port, tokenPromise, close } = await startAuthServer();
    cleanup = close;

    const response = await fetch(`http://127.0.0.1:${port}/callback?token=nt_session_test123`);

    expect(response.status).toBe(200);

    const token = await tokenPromise;
    expect(token).toBe('nt_session_test123');
  });

  it('returns 400 when callback is missing the token param', async () => {
    const { port, close } = await startAuthServer();
    cleanup = close;

    const response = await fetch(`http://127.0.0.1:${port}/callback`);

    expect(response.status).toBe(400);
  });

  it('returns 404 for non-callback paths', async () => {
    const { port, close } = await startAuthServer();
    cleanup = close;

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

  it('close() can be called safely even after server already shut down', async () => {
    const { port, tokenPromise, close } = await startAuthServer();

    await fetch(`http://127.0.0.1:${port}/callback?token=nt_session_abc`);
    await tokenPromise;

    // Should not throw
    await close();
    await close();
  });
});
