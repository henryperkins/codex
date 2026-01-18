import { describe, it, expect } from 'vitest';
import { splitMarkdownByBytes } from '../../src/ai-search/index.js';

describe('splitMarkdownByBytes', () => {
  it('returns a single part when under the limit', () => {
    const markdown = '# Title\n\nShort content.';
    const parts = splitMarkdownByBytes(markdown, 1024);
    expect(parts.length).toBe(1);
    expect(parts[0]).toBe(markdown);
  });

  it('keeps parts within the byte limit', () => {
    const paragraphs = Array.from({ length: 8 }, (_, idx) =>
      `Paragraph ${idx}\n` + 'word '.repeat(40)
    );
    const markdown = paragraphs.join('\n\n');
    const parts = splitMarkdownByBytes(markdown, 200);

    expect(parts.length).toBeGreaterThan(1);
    for (const part of parts) {
      expect(Buffer.byteLength(part, 'utf8')).toBeLessThanOrEqual(200);
      expect(part.length).toBeGreaterThan(0);
    }
  });

  it('handles multibyte characters safely', () => {
    const markdown = '😀'.repeat(50);
    const parts = splitMarkdownByBytes(markdown, 40);

    expect(parts.length).toBeGreaterThan(1);
    for (const part of parts) {
      expect(Buffer.byteLength(part, 'utf8')).toBeLessThanOrEqual(40);
      expect(part.length).toBeGreaterThan(0);
    }
  });
});
