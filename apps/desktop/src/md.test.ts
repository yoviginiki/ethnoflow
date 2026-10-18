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

describe("md tables and quotes", () => {
  it("parses a pipe table with escaped pipes, quotes and deep headings", () => {
    const row = "| A `x \\| y` | B |";
    const b = parseMarkdown(["#### Deep", "", "> note one", "> continues", "", row, "|---|---|", "| 1 | **2** |", "| 3 | 4 |", "", "text"].join("\n"));
    expect(b[0]).toMatchObject({ kind: "heading", level: 4, text: "Deep" });
    expect(b[1]).toMatchObject({ kind: "quote" });
    expect((b[1] as { inlines: { text: string }[] }).inlines.map((i) => i.text).join("")).toBe("note one continues");
    const t = b[2] as { kind: string; header: { text: string }[][]; rows: { text: string }[][][] };
    expect(t.kind).toBe("table");
    expect(t.header[0].map((i) => i.text).join("")).toBe("A x | y");
    expect(t.rows.length).toBe(2);
    expect(t.rows[0][1][0]).toMatchObject({ kind: "bold", text: "2" });
    expect(b[3]).toMatchObject({ kind: "paragraph" });
  });

  it("sections can split at ### too", () => {
    const s = sections(parseMarkdown("## Част 1\n### Дупки\nx\n### Регистри\ny"), 3);
    expect(s.map((x) => x.title)).toEqual(["Част 1", "Дупки", "Регистри"]);
    expect(s[2].text).toContain("y");
  });
});

