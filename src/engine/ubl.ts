import { documentAdjustmentTotals, prepaidAmountOf } from "../domain/calculate.js";
import { money } from "../domain/money.js";
import { assertValidInvoice } from "../domain/validate.js";
import { resolvedPrecedingInvoices, type AllowanceCharge, type CalculatedInvoice, type InvoiceLine, type Party } from "../domain/types.js";
import { element as e, xml, XML_HEADER } from "./xml.js";

export const XRECHNUNG_CUSTOMIZATION_ID = "urn:cen.eu:en16931:2017#compliant#urn:xeinkauf.de:kosit:xrechnung_3.0";
export const XRECHNUNG_PROFILE_ID = "urn:fdc:peppol.eu:2017:poacc:billing:01:1.0";

function partyXml(wrapper: string, party: Party): string {
  const endpoint = party.electronicAddress ? e("cbc:EndpointID", party.electronicAddress.value, ` schemeID="${xml(party.electronicAddress.schemeId)}"`) : "";
  const contact = party.contact ? `<cac:Contact>${e("cbc:Name", party.contact.name)}${e("cbc:Telephone", party.contact.phone)}${e("cbc:ElectronicMail", party.contact.email)}</cac:Contact>` : "";
  return `<cac:${wrapper}><cac:Party>${endpoint}<cac:PartyName>${e("cbc:Name", party.name)}</cac:PartyName><cac:PostalAddress>${e("cbc:StreetName", party.address.line1)}${e("cbc:AdditionalStreetName", party.address.line2)}${e("cbc:CityName", party.address.city)}${e("cbc:PostalZone", party.address.postalCode)}<cac:Country>${e("cbc:IdentificationCode", party.address.countryCode)}</cac:Country></cac:PostalAddress>${party.vatId ? `<cac:PartyTaxScheme>${e("cbc:CompanyID", party.vatId)}<cac:TaxScheme>${e("cbc:ID", "VAT")}</cac:TaxScheme></cac:PartyTaxScheme>` : ""}<cac:PartyLegalEntity>${e("cbc:RegistrationName", party.name)}</cac:PartyLegalEntity>${contact}</cac:Party></cac:${wrapper}>`;
}

function taxCategoryInner(tax: AllowanceCharge["tax"] | CalculatedInvoice["taxes"][number], withExemption: boolean): string {
  return `${e("cbc:ID", tax.categoryCode)}${e("cbc:Percent", tax.rate)}${withExemption ? `${e("cbc:TaxExemptionReasonCode", tax.exemptionReasonCode?.toUpperCase())}${e("cbc:TaxExemptionReason", tax.exemptionReason)}` : ""}<cac:TaxScheme>${e("cbc:ID", "VAT")}</cac:TaxScheme>`;
}

function allowanceXml(item: AllowanceCharge, currency: string, withTax = true): string {
  const tax = withTax
    ? `<cac:TaxCategory>${taxCategoryInner(item.tax, true)}</cac:TaxCategory>`
    : "";
  return `<cac:AllowanceCharge>${e("cbc:ChargeIndicator", item.charge ? "true" : "false")}${e("cbc:AllowanceChargeReasonCode", item.reasonCode)}${e("cbc:AllowanceChargeReason", item.reason)}${e("cbc:MultiplierFactorNumeric", item.percent)}${e("cbc:Amount", item.amount, ` currencyID="${currency}"`)}${e("cbc:BaseAmount", item.baseAmount, ` currencyID="${currency}"`)}${tax}</cac:AllowanceCharge>`;
}

function billingReferenceXml(invoice: CalculatedInvoice): string {
  return resolvedPrecedingInvoices(invoice).map((item) =>
    `<cac:BillingReference><cac:InvoiceDocumentReference>${e("cbc:ID", item.invoiceNumber)}${e("cbc:IssueDate", item.issueDate)}</cac:InvoiceDocumentReference></cac:BillingReference>`).join("");
}

function lineXml(invoice: CalculatedInvoice, line: InvoiceLine & { netAmount: string }, creditNote: boolean): string {
  const c = invoice.currency;
  const tag = creditNote ? "CreditNoteLine" : "InvoiceLine";
  const quantityTag = creditNote ? "CreditedQuantity" : "InvoicedQuantity";
  const allowances = (line.allowances ?? []).map((item) => allowanceXml(item, c, false)).join("");
  return `<cac:${tag}>${e("cbc:ID", line.id)}${e(`cbc:${quantityTag}`, line.quantity, ` unitCode="${xml(line.unitCode)}"`)}${e("cbc:LineExtensionAmount", line.netAmount, ` currencyID="${c}"`)}${allowances}<cac:Item>${e("cbc:Description", line.description)}${e("cbc:Name", line.name)}<cac:ClassifiedTaxCategory>${taxCategoryInner(line.tax, false)}</cac:ClassifiedTaxCategory></cac:Item><cac:Price>${e("cbc:PriceAmount", line.netUnitPrice, ` currencyID="${c}"`)}${e("cbc:BaseQuantity", "1", ` unitCode="${xml(line.unitCode)}"`)}</cac:Price></cac:${tag}>`;
}

export function generateUbl(invoice: CalculatedInvoice): string {
  assertValidInvoice(invoice);
  const c = invoice.currency;
  const creditNote = invoice.invoiceType === "381";
  const root = creditNote ? "CreditNote" : "Invoice";
  const xmlns = creditNote
    ? "urn:oasis:names:specification:ubl:schema:xsd:CreditNote-2"
    : "urn:oasis:names:specification:ubl:schema:xsd:Invoice-2";
  const typeTag = creditNote ? "CreditNoteTypeCode" : "InvoiceTypeCode";
  const { allowanceTotal, chargeTotal } = documentAdjustmentTotals(invoice.allowances);
  const prepaid = prepaidAmountOf(invoice);
  const taxes = invoice.taxes.map((tax) => `<cac:TaxSubtotal>${e("cbc:TaxableAmount", tax.taxableAmount, ` currencyID="${c}"`)}${e("cbc:TaxAmount", tax.taxAmount, ` currencyID="${c}"`)}<cac:TaxCategory>${taxCategoryInner(tax, true)}</cac:TaxCategory></cac:TaxSubtotal>`).join("");
  const allowances = (invoice.allowances ?? []).map((item) => allowanceXml(item, c)).join("");
  const lines = invoice.calculatedLines.map((line) => lineXml(invoice, line, creditNote)).join("");
  return `${XML_HEADER}<${root} xmlns="${xmlns}" xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2">${e("cbc:CustomizationID", XRECHNUNG_CUSTOMIZATION_ID)}${e("cbc:ProfileID", XRECHNUNG_PROFILE_ID)}${e("cbc:ID", invoice.invoiceNumber)}${e("cbc:IssueDate", invoice.issueDate)}${creditNote ? "" : e("cbc:DueDate", invoice.dueDate)}${e(`cbc:${typeTag}`, invoice.invoiceType)}${invoice.notes?.map((note) => e("cbc:Note", note)).join("") ?? ""}${e("cbc:DocumentCurrencyCode", c)}${e("cbc:BuyerReference", invoice.buyerReference)}${billingReferenceXml(invoice)}${partyXml("AccountingSupplierParty", invoice.seller)}${partyXml("AccountingCustomerParty", invoice.buyer)}${invoice.serviceDate || invoice.deliveryAddress ? `<cac:Delivery>${e("cbc:ActualDeliveryDate", invoice.serviceDate)}${invoice.deliveryAddress ? `<cac:DeliveryLocation><cac:Address>${e("cbc:StreetName", invoice.deliveryAddress.line1 || undefined)}${e("cbc:CityName", invoice.deliveryAddress.city || undefined)}${e("cbc:PostalZone", invoice.deliveryAddress.postalCode || undefined)}<cac:Country>${e("cbc:IdentificationCode", invoice.deliveryAddress.countryCode)}</cac:Country></cac:Address></cac:DeliveryLocation>` : ""}</cac:Delivery>` : ""}${invoice.payment.iban ? `<cac:PaymentMeans>${e("cbc:PaymentMeansCode", invoice.payment.meansCode)}${creditNote ? e("cbc:PaymentDueDate", invoice.dueDate) : ""}${e("cbc:PaymentID", invoice.payment.paymentReference)}<cac:PayeeFinancialAccount>${e("cbc:ID", invoice.payment.iban)}${e("cbc:Name", invoice.payment.accountName)}${invoice.payment.bic ? `<cac:FinancialInstitutionBranch>${e("cbc:ID", invoice.payment.bic)}</cac:FinancialInstitutionBranch>` : ""}</cac:PayeeFinancialAccount></cac:PaymentMeans>` : ""}${invoice.payment.terms ? `<cac:PaymentTerms>${e("cbc:Note", invoice.payment.terms)}</cac:PaymentTerms>` : ""}${allowances}<cac:TaxTotal>${e("cbc:TaxAmount", invoice.totals.taxTotal, ` currencyID="${c}"`)}${taxes}</cac:TaxTotal><cac:LegalMonetaryTotal>${e("cbc:LineExtensionAmount", invoice.totals.lineNet, ` currencyID="${c}"`)}${e("cbc:TaxExclusiveAmount", invoice.totals.taxExclusive, ` currencyID="${c}"`)}${e("cbc:TaxInclusiveAmount", invoice.totals.taxInclusive, ` currencyID="${c}"`)}${allowanceTotal.gt(0) ? e("cbc:AllowanceTotalAmount", money(allowanceTotal), ` currencyID="${c}"`) : ""}${chargeTotal.gt(0) ? e("cbc:ChargeTotalAmount", money(chargeTotal), ` currencyID="${c}"`) : ""}${prepaid.gt(0) ? e("cbc:PrepaidAmount", money(prepaid), ` currencyID="${c}"`) : ""}${e("cbc:PayableAmount", invoice.totals.payable, ` currencyID="${c}"`)}</cac:LegalMonetaryTotal>${lines}</${root}>`;
}
