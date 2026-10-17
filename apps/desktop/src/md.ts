/** A tiny Markdown subset for the in-app guide: headings, paragraphs, lists, bold, code. */
export type Inline = { kind: "text"; text: string } | { kind: "bold"; text: string } | { kind: "code"; text: string };
export type Block =
  | { kind: "heading"; level: number; text: string; id: string }
  | { kind: "paragraph"; inlines: Inline[] }
  | { kind: "list"; ordered: boolean; items: Inline[][] }
  | { kind: "code"; text: string };

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

export function parseMarkdown(md: string): Block[] {
  const lines = md.split(/\r?\n/);
  const blocks: Block[] = [];
  let para: string[] = [];
  let list: { ordered: boolean; items: string[] } | null = null;
  let code: string[] | null = null;
  const flushPara = () => {
    if (para.length) blocks.push({ kind: "paragraph", inlines: parseInline(para.join(" ")) });
    para = [];
  };
  const flushList = () => {
    if (list) blocks.push({ kind: "list", ordered: list.ordered, items: list.items.map(parseInline) });
    list = null;
  };
  for (const raw of lines) {
    if (code) {
      if (raw.startsWith("```")) {
        blocks.push({ kind: "code", text: code.join("\n") });
        code = null;
      } else code.push(raw);
      continue;
    }
    const line = raw.trimEnd();
    if (line.startsWith("```")) {
      flushPara();
      flushList();
      code = [];
      continue;
    }
    const h = /^(#{1,4})\s+(.*)$/.exec(line);
    if (h) {
      flushPara();
      flushList();
      blocks.push({ kind: "heading", level: h[1].length, text: h[2].trim(), id: slug(h[2]) });
      continue;
    }
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
    if (line.trim() === "") {
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
  flushPara();
  flushList();
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

/** Split into sections at `##` headings (the `#` title and anything before the first `##` form section 0). */
export function sections(blocks: Block[]): Section[] {
  const out: Section[] = [];
  let cur: Section | null = null;
  const textOf = (b: Block): string => {
    if (b.kind === "heading") return b.text;
    if (b.kind === "code") return b.text;
    if (b.kind === "paragraph") return b.inlines.map((i) => i.text).join("");
    return b.items.map((it) => it.map((i) => i.text).join("")).join(" ");
  };
  for (const b of blocks) {
    if (b.kind === "heading" && b.level <= 2) {
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
  for (const s of out) s.text = (s.title + " " + s.blocks.map(textOf).join(" ")).toLowerCase();
  return out;
}
