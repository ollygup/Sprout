import { describe, expect, it } from "vitest";
import { formatNote, hasNote } from "./noteFormat";

describe("hasNote", () => {
  it("is false for null, undefined, empty, whitespace-only", () => {
    expect(hasNote(null)).toBe(false);
    expect(hasNote(undefined)).toBe(false);
    expect(hasNote("")).toBe(false);
    expect(hasNote("   ")).toBe(false);
    expect(hasNote("\n\t  ")).toBe(false);
  });

  it("is true for any trimmed non-empty", () => {
    expect(hasNote("hello")).toBe(true);
    expect(hasNote("  hello  ")).toBe(true);
    expect(hasNote("0")).toBe(true);
  });
});

describe("formatNote", () => {
  it("returns empty for null/empty/whitespace", () => {
    expect(formatNote(null)).toBe("");
    expect(formatNote("")).toBe("");
    expect(formatNote("   \n  ")).toBe("");
  });

  it("renders a single paragraph", () => {
    expect(formatNote("hello world")).toBe("<p>hello world</p>");
  });

  it("renders paragraphs split by a blank line", () => {
    expect(formatNote("hello\n\nworld")).toBe("<p>hello</p><p>world</p>");
  });

  it("joins single-newline paragraph lines with a space", () => {
    expect(formatNote("hello\nworld")).toBe("<p>hello world</p>");
  });

  it("renders - bullet lists", () => {
    expect(formatNote("- a\n- b\n- c")).toBe("<ul><li>a</li><li>b</li><li>c</li></ul>");
  });

  it("renders * bullet lists", () => {
    expect(formatNote("* a\n* b")).toBe("<ul><li>a</li><li>b</li></ul>");
  });

  it("renders mixed - and * as one bullet list", () => {
    expect(formatNote("- a\n* b\n- c")).toBe("<ul><li>a</li><li>b</li><li>c</li></ul>");
  });

  it("renders ordered lists with 1. style", () => {
    expect(formatNote("1. first\n2. second\n3. third")).toBe(
      "<ol><li>first</li><li>second</li><li>third</li></ol>"
    );
  });

  it("renders ordered lists regardless of numbering sequence", () => {
    expect(formatNote("1. a\n1. b\n10. c")).toBe("<ol><li>a</li><li>b</li><li>c</li></ol>");
  });

  it("renders paragraphs and lists interleaved", () => {
    expect(formatNote("intro\n\n- a\n- b\n\noutro")).toBe(
      "<p>intro</p><ul><li>a</li><li>b</li></ul><p>outro</p>"
    );
    expect(formatNote("1. first\n2. second\n\nnote")).toBe(
      "<ol><li>first</li><li>second</li></ol><p>note</p>"
    );
  });

  it("handles list then paragraph without blank line", () => {
    expect(formatNote("- a\n- b\nhello")).toBe("<ul><li>a</li><li>b</li></ul><p>hello</p>");
  });

  it("handles paragraph then list without blank line", () => {
    expect(formatNote("hello\n- a\n- b")).toBe("<p>hello</p><ul><li>a</li><li>b</li></ul>");
  });

  it("escapes html in paragraphs", () => {
    expect(formatNote("<b>bold</b> & \"quotes\"")).toBe(
      "<p>&lt;b&gt;bold&lt;/b&gt; &amp; &quot;quotes&quot;</p>"
    );
  });

  it("escapes html in list items", () => {
    expect(formatNote("- <script>alert(1)</script>")).toBe(
      "<ul><li>&lt;script&gt;alert(1)&lt;/script&gt;</li></ul>"
    );
    expect(formatNote("1. a & b < c")).toBe("<ol><li>a &amp; b &lt; c</li></ol>");
  });

  it("leaves non-list markdown verbatim escaped", () => {
    expect(formatNote("# heading")).toBe("<p># heading</p>");
    expect(formatNote("**bold**")).toBe("<p>**bold**</p>");
    expect(formatNote("`code`")).toBe("<p>`code`</p>");
    expect(formatNote("[link](http://example.com)")).toBe("<p>[link](http://example.com)</p>");
    expect(formatNote("> quote")).toBe("<p>&gt; quote</p>");
  });

  it("requires space after bullet marker", () => {
    expect(formatNote("-nope")).toBe("<p>-nope</p>");
    expect(formatNote("*nope")).toBe("<p>*nope</p>");
  });

  it("requires space after ordered marker", () => {
    expect(formatNote("1.nope")).toBe("<p>1.nope</p>");
    expect(formatNote("1.")).toBe("<p>1.</p>");
  });

  it("trims surrounding whitespace but preserves internal blank-line structure", () => {
    expect(formatNote("  hello  \n\n  world  ")).toBe("<p>hello</p><p>world</p>");
  });

  it("handles CRLF line endings", () => {
    expect(formatNote("a\r\n\r\nb")).toBe("<p>a</p><p>b</p>");
    expect(formatNote("- a\r\n- b")).toBe("<ul><li>a</li><li>b</li></ul>");
  });

  it("is empty when only list markers without content are trimmed", () => {
    // "- " with no content after space is captured as "" -> still renders empty li?
    // Our regex requires at least one char after space, so "- " alone is not a list.
    expect(formatNote("- ")).toBe("<p>-</p>");
  });

  it("handles multiple blank lines as one separator", () => {
    expect(formatNote("a\n\n\nb")).toBe("<p>a</p><p>b</p>");
  });
});
