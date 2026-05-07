import { describe, it, expect } from 'vitest';
import { renderSchema } from './schema-render.js';

describe('renderSchema — required vs optional grouping', () => {
  it('groups required fields under "Required:" and optional under "Optional:"', () => {
    const lines = renderSchema({
      type: 'object',
      properties: {
        email: { type: 'string' },
        nickname: { type: 'string' },
      },
      required: ['email'],
    });

    const text = lines.join('\n');
    expect(text).toContain('Required:');
    expect(text).toContain('Optional:');
    const requiredIdx = lines.indexOf('Required:');
    const optionalIdx = lines.indexOf('Optional:');
    const emailIdx = lines.findIndex((l) => l.includes('email'));
    const nicknameIdx = lines.findIndex((l) => l.includes('nickname'));

    expect(requiredIdx).toBeGreaterThanOrEqual(0);
    expect(optionalIdx).toBeGreaterThan(requiredIdx);
    expect(emailIdx).toBeGreaterThan(requiredIdx);
    expect(emailIdx).toBeLessThan(optionalIdx);
    expect(nicknameIdx).toBeGreaterThan(optionalIdx);
  });

  it('omits "Optional:" header when there are no optional fields', () => {
    const lines = renderSchema({
      type: 'object',
      properties: { email: { type: 'string' } },
      required: ['email'],
    });

    expect(lines).not.toContain('Optional:');
  });

  it('omits "Required:" header when there are no required fields', () => {
    const lines = renderSchema({
      type: 'object',
      properties: { nickname: { type: 'string' } },
    });

    expect(lines).not.toContain('Required:');
  });
});

describe('renderSchema — type annotations', () => {
  it('annotates each field with its JSON-Schema type', () => {
    const lines = renderSchema({
      type: 'object',
      properties: {
        name: { type: 'string' },
        age: { type: 'integer' },
        active: { type: 'boolean' },
      },
    });

    const nameLine = lines.find((l) => l.includes('name'));
    const ageLine = lines.find((l) => l.includes('age'));
    const activeLine = lines.find((l) => l.includes('active'));
    expect(nameLine).toContain('string');
    expect(ageLine).toContain('integer');
    expect(activeLine).toContain('boolean');
  });

  it('annotates arrays with their item type ("array of <type>")', () => {
    const lines = renderSchema({
      type: 'object',
      properties: {
        tags: { type: 'array', items: { type: 'string' } },
      },
    });

    const tagsLine = lines.find((l) => l.includes('tags'));
    expect(tagsLine).toMatch(/array of string/);
  });

  it('annotates plain arrays without items as exactly "array" (no "of <type>" suffix)', () => {
    const lines = renderSchema({
      type: 'object',
      properties: { tags: { type: 'array' } },
    });

    const tagsLine = lines.find((l) => l.includes('tags'));
    expect(tagsLine).toBe('  tags: array');
  });

  it('annotates an array of enums with the enum signal', () => {
    const lines = renderSchema({
      type: 'object',
      properties: {
        levels: { type: 'array', items: { enum: ['debug', 'info', 'warn'] } },
      },
    });

    const levelsLine = lines.find((l) => l.includes('levels'));
    expect(levelsLine).toContain('debug');
    expect(levelsLine).toContain('info');
    expect(levelsLine).toContain('warn');
    expect(levelsLine).toContain('array of');
  });
});

describe('renderSchema — enums', () => {
  it('annotates string enums with type + "one of:" + JSON-encoded values', () => {
    const lines = renderSchema({
      type: 'object',
      properties: {
        plan: { type: 'string', enum: ['free', 'pro', 'enterprise'] },
      },
    });

    const planLine = lines.find((l) => l.includes('plan'));
    expect(planLine).toBe('  plan: string (one of: "free", "pro", "enterprise")');
  });

  it('falls back to the literal "enum" label when no type is provided', () => {
    const lines = renderSchema({
      type: 'object',
      properties: {
        mode: { enum: ['fast', 'safe'] },
      },
    });

    const modeLine = lines.find((l) => l.includes('mode'));
    expect(modeLine).toBe('  mode: enum (one of: "fast", "safe")');
  });
});

describe('renderSchema — empty object', () => {
  it('returns a single "(no fields)" line for an empty schema', () => {
    const lines = renderSchema({ type: 'object' });

    expect(lines.join('\n')).toMatch(/no fields/i);
  });
});

describe('renderSchema — non-object trust boundary', () => {
  it('describes a top-level scalar inline so signal is preserved', () => {
    const lines = renderSchema({ type: 'string' });

    expect(lines).toEqual(['(value: string)']);
  });

  it('returns "(no fields)" for a non-object input that carries no useful signal', () => {
    expect(renderSchema(null)).toEqual(['(no fields)']);
    expect(renderSchema('not-a-schema')).toEqual(['(no fields)']);
    expect(renderSchema([{ type: 'string' }])).toEqual(['(no fields)']);
  });
});
