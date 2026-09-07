import type { BoundingBox, DocumentPage, SourceToken, TextLine } from "./types.js";

function union(tokens: SourceToken[]): BoundingBox {
  const left = Math.min(...tokens.map((token) => token.box.x));
  const top = Math.min(...tokens.map((token) => token.box.y));
  const right = Math.max(...tokens.map((token) => token.box.x + token.box.width));
  const bottom = Math.max(...tokens.map((token) => token.box.y + token.box.height));
  return { x: left, y: top, width: right - left, height: bottom - top };
}

export function reconstructLines(pages: DocumentPage[], yTolerance = 3): TextLine[] {
  const result: TextLine[] = [];
  for (const page of pages) {
    const rows: SourceToken[][] = [];
    const sorted = [...page.tokens].sort((a, b) => a.box.y - b.box.y || a.box.x - b.box.x);
    for (const token of sorted) {
      const center = token.box.y + token.box.height / 2;
      const row = rows.find((candidate) => {
        const first = candidate[0];
        return first !== undefined && Math.abs(center - (first.box.y + first.box.height / 2)) <= Math.max(yTolerance, token.box.height * 0.35);
      });
      if (row) row.push(token); else rows.push([token]);
    }
    rows.forEach((row, index) => {
      row.sort((a, b) => a.box.x - b.box.x);
      result.push({ id: `p${page.page}-l${index}`, page: page.page, text: row.map((token) => token.text).join(" "), box: union(row), tokenIds: row.map((token) => token.id) });
    });
  }
  return result;
}

export interface TableRow {
  page: number;
  cells: Array<{ text: string; tokenIds: string[]; box: BoundingBox }>;
}

export function reconstructTableRows(lines: TextLine[], pages: DocumentPage[], minimumGap = 18): TableRow[] {
  const tokens = new Map(pages.flatMap((page) => page.tokens.map((token) => [token.id, token] as const)));
  return lines.map((line) => {
    const rowTokens = line.tokenIds.map((id) => tokens.get(id)).filter((token): token is SourceToken => token !== undefined);
    const groups: SourceToken[][] = [];
    for (const token of rowTokens) {
      const previous = groups.at(-1);
      const last = previous?.at(-1);
      if (!last || token.box.x - (last.box.x + last.box.width) >= minimumGap) groups.push([token]);
      else previous!.push(token);
    }
    return { page: line.page, cells: groups.map((group) => ({ text: group.map((token) => token.text).join(" "), tokenIds: group.map((token) => token.id), box: union(group) })) };
  });
}
