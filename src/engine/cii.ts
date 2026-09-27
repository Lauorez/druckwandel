import { documentAdjustmentTotals, prepaidAmountOf } from "../domain/calculate.js";
import { money } from "../domain/money.js";
import { assertValidEn16931Invoice } from "../domain/validate.js";
import { resolvedPrecedingInvoices, type AllowanceCharge, type CalculatedInvoice, type Party } from "../domain/types.js";
import { compactDate, element as e, xml, XML_HEADER } from "./xml.js";

// ZUGFeRD 2.5.x / Factur-X 1.09.x EN 16931 profile identifier.
export const ZUGFERD_EN16931_GUIDELINE = "urn:cen.eu:en16931:2017";

function partyXml(tag: string, party: Party): string {
  const tax = party.vatId ? `<ram:SpecifiedTaxRegistration>${e("ram:ID", party.vatId, ' schemeID="VA"')}</ram:SpecifiedTaxRegistration>` : "";
  const endpoint = party.electronicAddress
    ? `<ram:URIUniversalCommunication>${e("ram:URIID", party.electronicAddress.value, ` schemeID="${xml(party.electronicAddress.schemeId)}"`)}</ram:URIUniversalCommunication>`
    : "";
  const contact = party.contact ? `<ram:DefinedTradeContact>${e("ram:PersonName", party.contact.name)}${party.contact.phone ? `<ram:TelephoneUniversalCommunication>${e("ram:CompleteNumber", party.contact.phone)}</ram:TelephoneUniversalCommunication>` : ""}${party.contact.email ? `<ram:EmailURIUniversalCommunication>${e("ram:URIID", party.contact.email)}</ram:EmailURIUniversalCommunication>` : ""}</ram:DefinedTradeContact>` : "";
  return `<ram:${tag}>${e("ram:Name", party.name)}${contact}<ram:PostalTradeAddress>${e("ram:PostcodeCode", party.address.postalCode)}${e("ram:LineOne", party.address.line1)}${e("ram:LineTwo", party.address.line2)}${e("ram:CityName", party.address.city)}${e("ram:CountryID", party.address.countryCode)}</ram:PostalTradeAddress>${endpoint}${tax}</ram:${tag}>`;
}

function allowanceXml(item: AllowanceCharge, withTax: boolean): string {
  const tax = withTax
    ? `<ram:CategoryTradeTax>${e("ram:TypeCode", "VAT")}${e("ram:CategoryCode", item.tax.categoryCode)}${e("ram:RateApplicablePercent", item.tax.rate)}</ram:CategoryTradeTax>`
    : "";
  return `<ram:SpecifiedTradeAllowanceCharge><ram:ChargeIndicator><udt:Indicator>${item.charge ? "true" : "false"}</udt:Indicator></ram:ChargeIndicator>${e("ram:CalculationPercent", item.percent)}${e("ram:BasisAmount", item.baseAmount)}${e("ram:ActualAmount", item.amount)}${e("ram:ReasonCode", item.reasonCode)}${e("ram:Reason", item.reason)}${tax}</ram:SpecifiedTradeAllowanceCharge>`;
}

function referencedInvoiceXml(invoice: CalculatedInvoice): string {
  return resolvedPrecedingInvoices(invoice).map((item) => {
    const date = item.issueDate
      ? `<ram:FormattedIssueDateTime>${e("qdt:DateTimeString", compactDate(item.issueDate), ' format="102"')}</ram:FormattedIssueDateTime>`
      : "";
    return `<ram:InvoiceReferencedDocument>${e("ram:IssuerAssignedID", item.invoiceNumber)}${date}</ram:InvoiceReferencedDocument>`;
  }).join("");
}

export function generateCii(invoice: CalculatedInvoice): string {
  assertValidEn16931Invoice(invoice);
  const { allowanceTotal, chargeTotal } = documentAdjustmentTotals(invoice.allowances);
  const prepaid = prepaidAmountOf(invoice);
  const lines = invoice.calculatedLines.map((line) => {
    const allowances = (line.allowances ?? []).map((item) => allowanceXml(item, false)).join("");
    return `<ram:IncludedSupplyChainTradeLineItem><ram:AssociatedDocumentLineDocument>${e("ram:LineID", line.id)}</ram:AssociatedDocumentLineDocument><ram:SpecifiedTradeProduct>${e("ram:Name", line.name)}${e("ram:Description", line.description)}</ram:SpecifiedTradeProduct><ram:SpecifiedLineTradeAgreement><ram:NetPriceProductTradePrice>${e("ram:ChargeAmount", line.netUnitPrice)}</ram:NetPriceProductTradePrice></ram:SpecifiedLineTradeAgreement><ram:SpecifiedLineTradeDelivery>${e("ram:BilledQuantity", line.quantity, ` unitCode="${xml(line.unitCode)}"`)}</ram:SpecifiedLineTradeDelivery><ram:SpecifiedLineTradeSettlement><ram:ApplicableTradeTax>${e("ram:TypeCode", "VAT")}${e("ram:CategoryCode", line.tax.categoryCode)}${e("ram:RateApplicablePercent", line.tax.rate)}</ram:ApplicableTradeTax>${allowances}<ram:SpecifiedTradeSettlementLineMonetarySummation>${e("ram:LineTotalAmount", line.netAmount)}</ram:SpecifiedTradeSettlementLineMonetarySummation></ram:SpecifiedLineTradeSettlement></ram:IncludedSupplyChainTradeLineItem>`;
  }).join("");
  const taxes = invoice.taxes.map((tax) => `<ram:ApplicableTradeTax>${e("ram:CalculatedAmount", tax.taxAmount)}${e("ram:TypeCode", "VAT")}${e("ram:ExemptionReason", tax.exemptionReason)}${e("ram:BasisAmount", tax.taxableAmount)}${e("ram:CategoryCode", tax.categoryCode)}${e("ram:ExemptionReasonCode", tax.exemptionReasonCode?.toUpperCase())}${e("ram:RateApplicablePercent", tax.rate)}</ram:ApplicableTradeTax>`).join("");
  const headerAllowances = (invoice.allowances ?? []).map((item) => allowanceXml(item, true)).join("");
  const payment = `<ram:SpecifiedTradeSettlementPaymentMeans>${e("ram:TypeCode", invoice.payment.meansCode)}${invoice.payment.iban ? `<ram:PayeePartyCreditorFinancialAccount>${e("ram:IBANID", invoice.payment.iban)}${e("ram:AccountName", invoice.payment.accountName)}</ram:PayeePartyCreditorFinancialAccount>` : ""}${invoice.payment.bic ? `<ram:PayeeSpecifiedCreditorFinancialInstitution>${e("ram:BICID", invoice.payment.bic)}</ram:PayeeSpecifiedCreditorFinancialInstitution>` : ""}</ram:SpecifiedTradeSettlementPaymentMeans>`;
  const qdt = resolvedPrecedingInvoices(invoice).some((item) => item.issueDate) ? ' xmlns:qdt="urn:un:unece:uncefact:data:standard:QualifiedDataType:100"' : "";
  return `${XML_HEADER}<rsm:CrossIndustryInvoice xmlns:rsm="urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100" xmlns:ram="urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100" xmlns:udt="urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100"${qdt}><rsm:ExchangedDocumentContext><ram:GuidelineSpecifiedDocumentContextParameter>${e("ram:ID", ZUGFERD_EN16931_GUIDELINE)}</ram:GuidelineSpecifiedDocumentContextParameter></rsm:ExchangedDocumentContext><rsm:ExchangedDocument>${e("ram:ID", invoice.invoiceNumber)}${e("ram:TypeCode", invoice.invoiceType)}<ram:IssueDateTime>${e("udt:DateTimeString", compactDate(invoice.issueDate), ' format="102"')}</ram:IssueDateTime>${invoice.notes?.map((note) => `<ram:IncludedNote>${e("ram:Content", note)}</ram:IncludedNote>`).join("") ?? ""}</rsm:ExchangedDocument><rsm:SupplyChainTradeTransaction>${lines}<ram:ApplicableHeaderTradeAgreement>${e("ram:BuyerReference", invoice.buyerReference || undefined)}${partyXml("SellerTradeParty", invoice.seller)}${partyXml("BuyerTradeParty", invoice.buyer)}</ram:ApplicableHeaderTradeAgreement><ram:ApplicableHeaderTradeDelivery>${invoice.deliveryAddress ? `<ram:ShipToTradeParty><ram:PostalTradeAddress>${e("ram:PostcodeCode", invoice.deliveryAddress.postalCode || undefined)}${e("ram:LineOne", invoice.deliveryAddress.line1 || undefined)}${e("ram:CityName", invoice.deliveryAddress.city || undefined)}${e("ram:CountryID", invoice.deliveryAddress.countryCode)}</ram:PostalTradeAddress></ram:ShipToTradeParty>` : ""}${invoice.serviceDate ? `<ram:ActualDeliverySupplyChainEvent><ram:OccurrenceDateTime>${e("udt:DateTimeString", compactDate(invoice.serviceDate), ' format="102"')}</ram:OccurrenceDateTime></ram:ActualDeliverySupplyChainEvent>` : ""}</ram:ApplicableHeaderTradeDelivery><ram:ApplicableHeaderTradeSettlement>${e("ram:InvoiceCurrencyCode", invoice.currency)}${payment}${taxes}${headerAllowances}${invoice.payment.terms || invoice.dueDate ? `<ram:SpecifiedTradePaymentTerms>${e("ram:Description", invoice.payment.terms)}${invoice.dueDate ? `<ram:DueDateDateTime>${e("udt:DateTimeString", compactDate(invoice.dueDate), ' format="102"')}</ram:DueDateDateTime>` : ""}</ram:SpecifiedTradePaymentTerms>` : ""}<ram:SpecifiedTradeSettlementHeaderMonetarySummation>${e("ram:LineTotalAmount", invoice.totals.lineNet)}${chargeTotal.gt(0) ? e("ram:ChargeTotalAmount", money(chargeTotal)) : ""}${allowanceTotal.gt(0) ? e("ram:AllowanceTotalAmount", money(allowanceTotal)) : ""}${e("ram:TaxBasisTotalAmount", invoice.totals.taxExclusive)}${e("ram:TaxTotalAmount", invoice.totals.taxTotal, ` currencyID="${invoice.currency}"`)}${e("ram:GrandTotalAmount", invoice.totals.taxInclusive)}${prepaid.gt(0) ? e("ram:TotalPrepaidAmount", money(prepaid)) : ""}${e("ram:DuePayableAmount", invoice.totals.payable)}</ram:SpecifiedTradeSettlementHeaderMonetarySummation>${referencedInvoiceXml(invoice)}</ram:ApplicableHeaderTradeSettlement></rsm:SupplyChainTradeTransaction></rsm:CrossIndustryInvoice>`;
}
