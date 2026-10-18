/** A tiny Markdown subset for the in-app guide and the knowledge base: headings, paragraphs, lists, tables, quotes, bold, code. */
export type Inline = { kind: "text"; text: string } | { kind: "bold"; text: string } | { kind: "code"; text: string };
export type Block =
  | { kind: "heading"; level: number; text: string; id: string }
  | { kind: "paragraph"; inlines: Inline[] }
  | { kind: "list"; ordered: boolean; items: Inline[][] }
  | { kind: "code"; text: string }
  | { kind: "table"; header: Inline[][]; rows: Inline[][][] }
  | { kind: "quote"; inlines: Inline[] };

export function slug(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, "-")
    .replace(/^-+|-+$/g, "");
}

export function parseInline(text: string): Inline[] {
  const out: Inline[] = [];
  // Links keep their text; images are dropped.
  const src = text.replace(/!\[[^\]]*\]\([^)]*\)/g, "").replace(/\[([^\]]+)\]\([^)]*\)/g, "$1");
  const re = /(\*\*[^*]+\*\*|`[^`]+`)/g;
  let last = 0;
  for (const m of src.matchAll(re)) {
    const i = m.index ?? 0;
    if (i > last) out.push({ kind: "text", text: src.slice(last, i) });
    const tok = m[0];
    if (tok.startsWith("**")) out.push({ kind: "bold", text: tok.slice(2, -2) });
    else out.push({ kind: "code", text: tok.slice(1, -1) });
    last = i + tok.length;
  }
  if (last < src.length) out.push({ kind: "text", text: src.slice(last) });
  return out;
}

const ESC_PIPE = "\u0001";

/** Split a table row into cells; `\|` inside a cell stays a pipe. */
export function tableCells(line: string): string[] {
  const t = line.trim().replace(/\\\|/g, ESC_PIPE);
  const inner = t.startsWith("|") ? t.slice(1) : t;
  const body = inner.endsWith("|") ? inner.slice(0, -1) : inner;
  return body.split("|").map((c) => c.split(ESC_PIPE).join("|").trim());
}

const isTableRow = (line: string) => /^\s*\|.*\|\s*$/.test(line);
const isTableSep = (line: string) => /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/.test(line);

export function parseMarkdown(md: string): Block[] {
  const lines = md.split(/\r?\n/);
  const blocks: Block[] = [];
  let para: string[] = [];
  let quote: string[] = [];
  let list: { ordered: boolean; items: string[] } | null = null;
  let code: string[] | null = null;
  let table: { header: string[]; rows: string[][] } | null = null;
  const flushPara = () => {
    if (para.length) blocks.push({ kind: "paragraph", inlines: parseInline(para.join(" ")) });
    para = [];
  };
  const flushQuote = () => {
    if (quote.length) blocks.push({ kind: "quote", inlines: parseInline(quote.join(" ")) });
    quote = [];
  };
  const flushList = () => {
    if (list) blocks.push({ kind: "list", ordered: list.ordered, items: list.items.map(parseInline) });
    list = null;
  };
  const flushTable = () => {
    if (table) blocks.push({ kind: "table", header: table.header.map(parseInline), rows: table.rows.map((r) => r.map(parseInline)) });
    table = null;
  };
  const flushAll = () => {
    flushPara();
    flushQuote();
    flushList();
    flushTable();
  };
  for (let i = 0; i < lines.length; i++) {
    const raw = lines[i];
    if (code) {
      if (raw.startsWith("```")) {
        blocks.push({ kind: "code", text: code.join("\n") });
        code = null;
      } else code.push(raw);
      continue;
    }
    const line = raw.trimEnd();
    if (line.startsWith("```")) {
      flushAll();
      code = [];
      continue;
    }
    if (isTableRow(line)) {
      if (!table) {
        // A header row followed by a separator row starts a table; otherwise it is text.
        if (i + 1 < lines.length && isTableSep(lines[i + 1])) {
          flushAll();
          table = { header: tableCells(line), rows: [] };
          i++; // skip the separator
          continue;
        }
      } else {
        table.rows.push(tableCells(line));
        continue;
      }
    } else if (table) {
      flushTable();
    }
    const h = /^(#{1,6})\s+(.*)$/.exec(line);
    if (h) {
      flushAll();
      blocks.push({ kind: "heading", level: h[1].length, text: h[2].trim(), id: slug(h[2]) });
      continue;
    }
    const q = /^\s*>\s?(.*)$/.exec(line);
    if (q) {
      flushPara();
      flushList();
      if (q[1].trim() === "") flushQuote();
      else quote.push(q[1].trim());
      continue;
    }
    if (quote.length) flushQuote();
    const li = /^\s*(?:[-*]|\d+[.)])\s+(.*)$/.exec(line);
    if (li) {
      flushPara();
      const ordered = /^\s*\d/.test(line);
      if (!list || list.ordered !== ordered) {
        flushList();
        list = { ordered, items: [] };
      }
      list.items.push(li[1]);
      continue;
    }
    if (line.trim() === "" || line.trim() === "---") {
      flushPara();
      flushList();
      continue;
    }
    if (list && /^\s{2,}/.test(raw)) {
      // continuation of the previous list item
      list.items[list.items.length - 1] += " " + line.trim();
      continue;
    }
    flushList();
    para.push(line.trim());
  }
  flushAll();
  if (code) blocks.push({ kind: "code", text: (code as string[]).join("\n") });
  return blocks;
}

export interface Section {
  title: string;
  id: string;
  level: number;
  blocks: Block[];
  /** Lower-cased text of the whole section, for search. */
  text: string;
}

export function blockText(b: Block): string {
  switch (b.kind) {
    case "heading":
      return b.text;
    case "code":
      return b.text;
    case "paragraph":
    case "quote":
      return b.inlines.map((i) => i.text).join("");
    case "list":
      return b.items.map((it) => it.map((i) => i.text).join("")).join(" ");
    case "table":
      return [b.header, ...b.rows].map((r) => r.map((c) => c.map((i) => i.text).join("")).join(" ")).join(" ");
  }
}

/**
 * Split into sections at headings up to `maxLevel` (default `##`; the `#` title and
 * anything before the first `##` form section 0).
 */
export function sections(blocks: Block[], maxLevel = 2): Section[] {
  const out: Section[] = [];
  let cur: Section | null = null;
  for (const b of blocks) {
    if (b.kind === "heading" && b.level <= maxLevel) {
      cur = { title: b.text, id: b.id, level: b.level, blocks: [], text: "" };
      out.push(cur);
      continue;
    }
    if (!cur) {
      cur = { title: "", id: "top", level: 1, blocks: [], text: "" };
      out.push(cur);
    }
    cur.blocks.push(b);
  }
  for (const s of out) s.text = (s.title + " " + s.blocks.map(blockText).join(" ")).toLowerCase();
  return out;
}
