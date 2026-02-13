import { describe, test, expect } from 'vitest';
import { markdownToHtml, htmlToMarkdown, createTurndownService } from './wysiwygMarkdown';

describe('wysiwygMarkdown', () => {
  describe('attachment image roundtrip', () => {
    test('preserves attachment URL through markdown→HTML→markdown roundtrip', () => {
      const markdown = '![image](attachment:2ea23510-cef4-47ba-916f-9f91135629a9)';
      const html = markdownToHtml(markdown);
      const result = htmlToMarkdown(html);

      expect(result).toBe(markdown);
    });

    test('preserves attachment URL with alt text', () => {
      const markdown = '![my screenshot](attachment:abc-123-def)';
      const html = markdownToHtml(markdown);
      const result = htmlToMarkdown(html);

      expect(result).toBe(markdown);
    });

    test('preserves attachment URL with title', () => {
      const markdown = '![image](attachment:uuid-here "Image Title")';
      const html = markdownToHtml(markdown);
      const result = htmlToMarkdown(html);

      expect(result).toBe(markdown);
    });

    test('HTML output includes data-attachment-url attribute', () => {
      const markdown = '![image](attachment:test-uuid)';
      const html = markdownToHtml(markdown);

      expect(html).toContain('data-attachment-url="attachment:test-uuid"');
      expect(html).toContain('alt="image"');
    });

    test('HTML output uses placeholder src for attachment images', () => {
      const markdown = '![image](attachment:test-uuid)';
      const html = markdownToHtml(markdown);

      expect(html).toContain('src=""');
      expect(html).toContain('class="attachment-placeholder"');
    });

    test('preserves multiple attachment images', () => {
      const markdown = `Some text

![first](attachment:uuid-1)

More text

![second](attachment:uuid-2)`;
      const html = markdownToHtml(markdown);
      const result = htmlToMarkdown(html);

      expect(result).toContain('![first](attachment:uuid-1)');
      expect(result).toContain('![second](attachment:uuid-2)');
    });

    test('attachment image with empty alt text uses attachment ref', () => {
      const markdown = '![](attachment:some-uuid)';
      const html = markdownToHtml(markdown);

      // Alt should fall back to the attachment reference
      expect(html).toContain('alt="some-uuid"');
    });
  });

  describe('note link roundtrip', () => {
    test('preserves note link through roundtrip', () => {
      const markdown = '[My Note](link:note-uuid-123)';
      const html = markdownToHtml(markdown);
      const result = htmlToMarkdown(html);

      expect(result).toBe(markdown);
    });

    test('preserves empty note link with placeholder', () => {
      const markdown = '[](link:note-uuid)';
      const html = markdownToHtml(markdown);

      // Should use placeholder emoji
      expect(html).toContain('🔗');

      const result = htmlToMarkdown(html);
      expect(result).toBe(markdown);
    });

    test('preserves note link text and URL (title is not preserved)', () => {
      // Note: Turndown's link rule doesn't preserve titles by default
      // The important thing is that the link: URL and text are preserved
      const markdown = '[Note Title](link:uuid "Some title")';
      const html = markdownToHtml(markdown);
      const result = htmlToMarkdown(html);

      // Title is lost but link URL and text are preserved
      expect(result).toContain('link:uuid');
      expect(result).toContain('Note Title');
    });
  });

  describe('regular images', () => {
    test('preserves regular image URLs', () => {
      const markdown = '![alt text](https://example.com/image.png)';
      const html = markdownToHtml(markdown);
      const result = htmlToMarkdown(html);

      expect(result).toBe(markdown);
    });

    test('regular images do not have data-attachment-url', () => {
      const markdown = '![alt](https://example.com/image.png)';
      const html = markdownToHtml(markdown);

      expect(html).not.toContain('data-attachment-url');
      expect(html).toContain('src="https://example.com/image.png"');
    });
  });

  describe('mixed content', () => {
    test('preserves mixed content with attachments and regular images', () => {
      const markdown = `# Heading

![attachment](attachment:uuid-1)

Some paragraph text.

![regular](https://example.com/img.png)

[Note link](link:note-123)`;

      const html = markdownToHtml(markdown);
      const result = htmlToMarkdown(html);

      expect(result).toContain('![attachment](attachment:uuid-1)');
      expect(result).toContain('![regular](https://example.com/img.png)');
      expect(result).toContain('[Note link](link:note-123)');
    });
  });

  describe('createTurndownService', () => {
    test('creates service with attachment image rule', () => {
      const service = createTurndownService();
      const html = '<img src="blob:xxx" alt="test" data-attachment-url="attachment:my-uuid" />';
      const result = service.turndown(html);

      expect(result).toBe('![test](attachment:my-uuid)');
    });

    test('creates service with note link rule', () => {
      const service = createTurndownService();
      const html = '<a href="link:note-uuid">My Note</a>';
      const result = service.turndown(html);

      expect(result).toBe('[My Note](link:note-uuid)');
    });

    test('ignores src attribute when data-attachment-url is present', () => {
      const service = createTurndownService();
      // Simulates what Tiptap outputs - blob URL in src, original URL in data attribute
      const html = '<img src="blob:http://localhost/12345" alt="image" data-attachment-url="attachment:original-uuid" />';
      const result = service.turndown(html);

      // Should use the data-attachment-url, not the blob URL
      expect(result).toBe('![image](attachment:original-uuid)');
      expect(result).not.toContain('blob:');
    });
  });

  describe('edge cases', () => {
    test('handles empty markdown', () => {
      expect(markdownToHtml('')).toBe('');
      expect(htmlToMarkdown('')).toBe('');
    });

    test('handles empty paragraph HTML', () => {
      expect(htmlToMarkdown('<p></p>')).toBe('');
    });

    test('handles markdown with only whitespace', () => {
      // Whitespace-only input produces empty output
      const result = markdownToHtml('   \n\n   ');
      expect(result).toBe('');
    });

    test('handles special characters in alt text', () => {
      const markdown = '![image with "quotes" & special chars](attachment:uuid)';
      const html = markdownToHtml(markdown);
      const result = htmlToMarkdown(html);

      // The roundtrip should preserve the content
      expect(result).toContain('attachment:uuid');
    });
  });
});
