import type { BoundingBox, DocumentPage } from "../../../src/extraction/types.js";
import type { LearnableFieldName } from "../../../src/learning/correction-memory.js";
import type { ReviewDraft } from "../../../src/review/draft.js";

export function tokensInSelection(page: DocumentPage, box: BoundingBox): string[] {
  return page.tokens.filter((token) => {
    const overlapX = Math.min(box.x + box.width, token.box.x + token.box.width) - Math.max(box.x, token.box.x);
    const overlapY = Math.min(box.y + box.height, token.box.y + token.box.height) - Math.max(box.y, token.box.y);
    return overlapX > 0 && overlapY > 0;
  }).map((token) => token.id);
}

export function assignSourceValue(draft: ReviewDraft, field: LearnableFieldName, value: string): ReviewDraft {
  const partyKeys = {
    Name: "name", AddressLine1: "addressLine1", PostalCode: "postalCode",
    City: "city", CountryCode: "countryCode", VatId: "vatId",
  } as const;
  for (const party of ["seller", "buyer"] as const) {
    if (field.startsWith(party)) {
      const key = partyKeys[field.slice(party.length) as keyof typeof partyKeys];
      return { ...draft, [party]: { ...draft[party], [key]: value } };
    }
  }
  if (field === "iban" || field === "bic" || field === "paymentTerms") {
    return { ...draft, payment: { ...draft.payment, [field === "paymentTerms" ? "terms" : field]: value } };
  }
  return { ...draft, [field]: value };
}
