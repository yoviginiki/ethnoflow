import { describe, expect, it } from "vitest";
import { parseInline, parseMarkdown, sections } from "./md";

describe("md", () => {
  it("parses headings, paragraphs, lists and code", () => {
    const b = parseMarkdown("# Title\n\nIntro **bold** and `code`.\n\n## One\n- a\n- b\n\n1. x\n2. y\n\n```\nraw\n```\n");
    expect(b[0]).toMatchObject({ kind: "heading", level: 1, text: "Title" });
    expect(b[1]).toMatchObject({ kind: "paragraph" });
    expect(b[2]).toMatchObject({ kind: "heading", level: 2, text: "One", id: "one" });
    expect(b[3]).toMatchObject({ kind: "list", ordered: false });
    expect(b[4]).toMatchObject({ kind: "list", ordered: true });
    expect(b[5]).toMatchObject({ kind: "code", text: "raw" });
  });

  it("keeps link text, drops images, splits bold and code", () => {
    expect(parseInline("see [docs](x.md) ![i](p.png) **b** `c`")).toEqual([
      { kind: "text", text: "see docs  " },
      { kind: "bold", text: "b" },
      { kind: "text", text: " " },
      { kind: "code", text: "c" },
    ]);
  });

  it("splits sections at ## and indexes their text for search", () => {
    const s = sections(parseMarkdown("# G\nhello\n## Дрон\nниско **бръмчене**\n## Пад\nакорд"));
    expect(s.map((x) => x.title)).toEqual(["G", "Дрон", "Пад"]);
    expect(s[1].text).toContain("бръмчене");
    expect(s[1].id).toBe("дрон");
  });
});
