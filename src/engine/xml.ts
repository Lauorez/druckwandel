export const xml = (value: string): string => value
  .replaceAll("&", "&amp;")
  .replaceAll("<", "&lt;")
  .replaceAll(">", "&gt;")
  .replaceAll('"', "&quot;")
  .replaceAll("'", "&apos;");

export const compactDate = (date: string): string => date.replaceAll("-", "");

export function element(name: string, value: string | undefined, attributes = ""): string {
  return value === undefined ? "" : `<${name}${attributes}>${xml(value)}</${name}>`;
}

export const XML_HEADER = '<?xml version="1.0" encoding="UTF-8"?>';
