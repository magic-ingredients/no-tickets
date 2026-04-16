import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';

export interface StoredCredentials {
  readonly token: string;
  readonly email: string;
  readonly expiresAt: string;
}

function credentialsDir(): string {
  return path.join(os.homedir(), '.notickets');
}

function credentialsPath(): string {
  return path.join(credentialsDir(), 'credentials');
}

function isStoredCredentials(value: unknown): value is StoredCredentials {
  if (typeof value !== 'object' || value === null) return false;
  const obj = value as Record<string, unknown>;
  return (
    typeof obj['token'] === 'string' &&
    typeof obj['email'] === 'string' &&
    typeof obj['expiresAt'] === 'string'
  );
}

export function saveCredentials(token: string, email: string, expiresAt: string): void {
  const dir = credentialsDir();
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }

  const credentials: StoredCredentials = { token, email, expiresAt };
  const filePath = credentialsPath();
  fs.writeFileSync(filePath, JSON.stringify(credentials, null, 2), 'utf-8');

  if (os.platform() !== 'win32') {
    fs.chmodSync(filePath, 0o600);
  }
}

export function loadCredentials(): StoredCredentials | null {
  const filePath = credentialsPath();
  if (!fs.existsSync(filePath)) return null;

  try {
    const raw = fs.readFileSync(filePath, 'utf-8');
    const parsed: unknown = JSON.parse(raw);

    if (!isStoredCredentials(parsed)) return null;

    if (new Date(parsed.expiresAt).getTime() <= Date.now()) return null;

    return parsed;
  } catch {
    return null;
  }
}

export function clearCredentials(): void {
  const filePath = credentialsPath();
  if (fs.existsSync(filePath)) {
    fs.unlinkSync(filePath);
  }
}
