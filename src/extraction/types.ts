export interface BoundingBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface SourceToken {
  id: string;
  page: number;
  text: string;
  box: BoundingBox;
  origin: "text-layer" | "ocr";
}

export interface DocumentPage {
  page: number;
  width: number;
  height: number;
  tokens: SourceToken[];
}

export interface TextLine {
  id: string;
  page: number;
  text: string;
  box: BoundingBox;
  tokenIds: string[];
}

export type ExtractedFieldName =
  | "invoiceNumber" | "issueDate" | "dueDate" | "serviceDate"
  | "currency" | "buyerReference"
  | "sellerName" | "sellerAddressLine1" | "sellerPostalCode" | "sellerCity" | "sellerCountryCode" | "sellerVatId"
  | "sellerContact" | "sellerPhone" | "sellerEmail"
  | "buyerName" | "buyerAddressLine1" | "buyerPostalCode" | "buyerCity" | "buyerCountryCode" | "buyerVatId"
  | "iban" | "bic" | "paymentTerms"
  | "lineNet" | "taxTotal" | "taxInclusive" | "payable";

export interface TransformationStep {
  operation: string;
  input: string;
  output: string;
}

export interface ExtractedField {
  name: ExtractedFieldName;
  value: string;
  confidence: number;
  sourceTokenIds: string[];
  sourceText: string;
  transformations: TransformationStep[];
}

export interface ExtractionWarning {
  code: string;
  message: string;
  page?: number;
}

export interface ExtractionResult {
  pages: DocumentPage[];
  lines: TextLine[];
  fields: Partial<Record<ExtractedFieldName, ExtractedField>>;
  lineItems: ExtractedLineItem[];
  warnings: ExtractionWarning[];
  usedOcr: boolean;
}

export interface ExtractedLineItem {
  description: string;
  serviceDate?: string;
  quantity: string;
  unit?: string;
  netUnitPrice: string;
  netAmount: string;
  taxRate?: string;
  confidence: number;
  sourceTokenIds: string[];
  sourceText: string;
}

export interface OcrAdapter {
  readonly name: string;
  recognize(pdf: Uint8Array): Promise<DocumentPage[]>;
}
