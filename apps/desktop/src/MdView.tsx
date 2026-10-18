import type { Block, Inline } from "./md";

export function Inlines({ items }: { items: Inline[] }) {
  return (
    <>
      {items.map((i, k) =>
        i.kind === "bold" ? <strong key={k}>{i.text}</strong> : i.kind === "code" ? <code key={k}>{i.text}</code> : <span key={k}>{i.text}</span>,
      )}
    </>
  );
}

/** One parsed Markdown block (shared by Help and the Academy knowledge base). */
export function BlockView({ b }: { b: Block }) {
  switch (b.kind) {
    case "heading": {
      const Tag = (`h${Math.min(6, b.level + 1)}` as unknown) as "h3";
      return <Tag id={b.id}>{b.text}</Tag>;
    }
    case "paragraph":
      return (
        <p>
          <Inlines items={b.inlines} />
        </p>
      );
    case "quote":
      return (
        <blockquote>
          <Inlines items={b.inlines} />
        </blockquote>
      );
    case "list":
      return b.ordered ? (
        <ol>
          {b.items.map((it, i) => (
            <li key={i}>
              <Inlines items={it} />
            </li>
          ))}
        </ol>
      ) : (
        <ul>
          {b.items.map((it, i) => (
            <li key={i}>
              <Inlines items={it} />
            </li>
          ))}
        </ul>
      );
    case "table":
      return (
        <div className="md-table-wrap">
          <table className="md-table">
            <thead>
              <tr>
                {b.header.map((c, i) => (
                  <th key={i}>
                    <Inlines items={c} />
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {b.rows.map((r, i) => (
                <tr key={i}>
                  {r.map((c, j) => (
                    <td key={j}>
                      <Inlines items={c} />
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      );
    case "code":
      return <pre>{b.text}</pre>;
  }
}
