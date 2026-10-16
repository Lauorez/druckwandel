use crate::invoice::{AllowanceCharge, InvoiceInput, Party, calculate, preceding, validate};
use crate::ubl::{e, element, escape, optional};

fn date(tag: &str, value: &str) -> String {
    element(tag, Some(&value.replace('-', "")), " format=\"102\"")
}

fn party(tag: &str, party: &Party) -> String {
    let mut xml = e("ram:Name", &party.name);
    if let Some(contact) = &party.contact {
        xml += "<ram:DefinedTradeContact>";
        xml += &optional("ram:PersonName", contact.name.as_deref());
        if let Some(phone) = &contact.phone {
            xml += &format!(
                "<ram:TelephoneUniversalCommunication>{}</ram:TelephoneUniversalCommunication>",
                e("ram:CompleteNumber", phone)
            );
        }
        if let Some(email) = &contact.email {
            xml += &format!(
                "<ram:EmailURIUniversalCommunication>{}</ram:EmailURIUniversalCommunication>",
                e("ram:URIID", email)
            );
        }
        xml += "</ram:DefinedTradeContact>";
    }
    xml += "<ram:PostalTradeAddress>";
    xml += &e("ram:PostcodeCode", &party.address.postal_code);
    xml += &e("ram:LineOne", &party.address.line1);
    xml += &optional("ram:LineTwo", party.address.line2.as_deref());
    xml += &e("ram:CityName", &party.address.city);
    xml += &e("ram:CountryID", &party.address.country_code);
    xml += "</ram:PostalTradeAddress>";
    if let Some(endpoint) = &party.electronic_address {
        xml += &format!(
            "<ram:URIUniversalCommunication>{}</ram:URIUniversalCommunication>",
            element(
                "ram:URIID",
                Some(&endpoint.value),
                &format!(" schemeID=\"{}\"", escape(&endpoint.scheme_id))
            )
        );
    }
    if let Some(vat) = &party.vat_id {
        xml += &format!(
            "<ram:SpecifiedTaxRegistration>{}</ram:SpecifiedTaxRegistration>",
            element("ram:ID", Some(vat), " schemeID=\"VA\"")
        );
    }
    if let Some(id) = &party.tax_registration_id {
        xml += &format!(
            "<ram:SpecifiedTaxRegistration>{}</ram:SpecifiedTaxRegistration>",
            element("ram:ID", Some(id), " schemeID=\"FC\"")
        );
    }
    format!("<ram:{tag}>{xml}</ram:{tag}>")
}

fn allowance(item: &AllowanceCharge, with_tax: bool) -> String {
    let mut xml = format!(
        "<ram:SpecifiedTradeAllowanceCharge><ram:ChargeIndicator><udt:Indicator>{}</udt:Indicator></ram:ChargeIndicator>",
        item.charge
    );
    xml += &optional("ram:CalculationPercent", item.percent.as_deref());
    xml += &optional("ram:BasisAmount", item.base_amount.as_deref());
    xml += &e("ram:ActualAmount", &item.amount);
    xml += &optional("ram:ReasonCode", item.reason_code.as_deref());
    xml += &optional("ram:Reason", item.reason.as_deref());
    if with_tax {
        xml += &format!(
            "<ram:CategoryTradeTax>{}{}{}</ram:CategoryTradeTax>",
            e("ram:TypeCode", "VAT"),
            e("ram:CategoryCode", &item.tax.category_code),
            e("ram:RateApplicablePercent", &item.tax.rate)
        );
    }
    xml + "</ram:SpecifiedTradeAllowanceCharge>"
}

/// ZUGFeRD / Factur-X EN 16931, using the same canonical invoice and rounding as UBL.
pub fn generate_cii(input: InvoiceInput) -> Result<String, String> {
    let errors = validate(&input);
    if !errors.is_empty() {
        return Err(errors.join(" "));
    }
    let invoice = calculate(input)?;
    if invoice.totals.payable.starts_with('-') {
        return Err("Der Zahlbetrag darf nicht negativ sein.".into());
    }
    let data = &invoice.input;
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><rsm:CrossIndustryInvoice xmlns:rsm=\"urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100\" xmlns:ram=\"urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100\" xmlns:udt=\"urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100\" xmlns:qdt=\"urn:un:unece:uncefact:data:standard:QualifiedDataType:100\"><rsm:ExchangedDocumentContext><ram:GuidelineSpecifiedDocumentContextParameter><ram:ID>urn:cen.eu:en16931:2017</ram:ID></ram:GuidelineSpecifiedDocumentContextParameter></rsm:ExchangedDocumentContext><rsm:ExchangedDocument>",
    );
    xml += &e("ram:ID", &data.invoice_number);
    xml += &e("ram:TypeCode", &data.invoice_type);
    xml += &format!(
        "<ram:IssueDateTime>{}</ram:IssueDateTime>",
        date("udt:DateTimeString", &data.issue_date)
    );
    for note in &data.notes {
        xml += &format!(
            "<ram:IncludedNote>{}</ram:IncludedNote>",
            e("ram:Content", note)
        );
    }
    xml += "</rsm:ExchangedDocument><rsm:SupplyChainTradeTransaction>";
    for calculated in &invoice.lines {
        let line = &calculated.line;
        xml += &format!(
            "<ram:IncludedSupplyChainTradeLineItem><ram:AssociatedDocumentLineDocument>{}</ram:AssociatedDocumentLineDocument><ram:SpecifiedTradeProduct>{}{}</ram:SpecifiedTradeProduct><ram:SpecifiedLineTradeAgreement><ram:NetPriceProductTradePrice>{}</ram:NetPriceProductTradePrice></ram:SpecifiedLineTradeAgreement><ram:SpecifiedLineTradeDelivery>{}</ram:SpecifiedLineTradeDelivery><ram:SpecifiedLineTradeSettlement><ram:ApplicableTradeTax>{}{}{}</ram:ApplicableTradeTax>",
            e("ram:LineID", &line.id),
            e("ram:Name", &line.name),
            optional("ram:Description", line.description.as_deref()),
            e("ram:ChargeAmount", &line.net_unit_price),
            element(
                "ram:BilledQuantity",
                Some(&line.quantity),
                &format!(" unitCode=\"{}\"", escape(&line.unit_code))
            ),
            e("ram:TypeCode", "VAT"),
            e("ram:CategoryCode", &line.tax.category_code),
            e("ram:RateApplicablePercent", &line.tax.rate)
        );
        for adjustment in &line.allowances {
            xml += &allowance(adjustment, false);
        }
        xml += &format!(
            "<ram:SpecifiedTradeSettlementLineMonetarySummation>{}</ram:SpecifiedTradeSettlementLineMonetarySummation></ram:SpecifiedLineTradeSettlement></ram:IncludedSupplyChainTradeLineItem>",
            e("ram:LineTotalAmount", &calculated.net_amount)
        );
    }
    xml += "<ram:ApplicableHeaderTradeAgreement>";
    if !data.buyer_reference.is_empty() {
        xml += &e("ram:BuyerReference", &data.buyer_reference);
    }
    xml += &party("SellerTradeParty", &data.seller);
    xml += &party("BuyerTradeParty", &data.buyer);
    xml += "</ram:ApplicableHeaderTradeAgreement><ram:ApplicableHeaderTradeDelivery>";
    if let Some(address) = &data.delivery_address {
        xml += &format!(
            "<ram:ShipToTradeParty><ram:PostalTradeAddress>{}{}{}{}</ram:PostalTradeAddress></ram:ShipToTradeParty>",
            e("ram:PostcodeCode", &address.postal_code),
            e("ram:LineOne", &address.line1),
            e("ram:CityName", &address.city),
            e("ram:CountryID", &address.country_code)
        );
    }
    if let Some(value) = &data.service_date {
        xml += &format!(
            "<ram:ActualDeliverySupplyChainEvent><ram:OccurrenceDateTime>{}</ram:OccurrenceDateTime></ram:ActualDeliverySupplyChainEvent>",
            date("udt:DateTimeString", value)
        );
    }
    xml += "</ram:ApplicableHeaderTradeDelivery><ram:ApplicableHeaderTradeSettlement>";
    xml += &e("ram:InvoiceCurrencyCode", &data.currency);
    xml += "<ram:SpecifiedTradeSettlementPaymentMeans>";
    xml += &e("ram:TypeCode", &data.payment.means_code);
    if let Some(iban) = &data.payment.iban {
        xml += &format!(
            "<ram:PayeePartyCreditorFinancialAccount>{}{}</ram:PayeePartyCreditorFinancialAccount>",
            e("ram:IBANID", iban),
            optional("ram:AccountName", data.payment.account_name.as_deref())
        );
    }
    if let Some(bic) = &data.payment.bic {
        xml += &format!(
            "<ram:PayeeSpecifiedCreditorFinancialInstitution>{}</ram:PayeeSpecifiedCreditorFinancialInstitution>",
            e("ram:BICID", bic)
        );
    }
    xml += "</ram:SpecifiedTradeSettlementPaymentMeans>";
    for group in &invoice.taxes {
        let tax = &group.tax;
        let rate = tax
            .rate
            .parse::<rust_decimal::Decimal>()
            .map_err(|e| e.to_string())?;
        xml += &format!(
            "<ram:ApplicableTradeTax>{}{}{}{}{}{}{}</ram:ApplicableTradeTax>",
            e("ram:CalculatedAmount", &group.tax_amount),
            e("ram:TypeCode", "VAT"),
            optional("ram:ExemptionReason", tax.exemption_reason.as_deref()),
            e("ram:BasisAmount", &group.taxable_amount),
            e("ram:CategoryCode", &tax.category_code),
            optional(
                "ram:ExemptionReasonCode",
                tax.exemption_reason_code
                    .as_ref()
                    .map(|s| s.to_uppercase())
                    .as_deref()
            ),
            e("ram:RateApplicablePercent", &format!("{rate:.2}"))
        );
    }
    for adjustment in &data.allowances {
        xml += &allowance(adjustment, true);
    }
    if data.payment.terms.is_some() || data.due_date.is_some() {
        xml += "<ram:SpecifiedTradePaymentTerms>";
        xml += &optional("ram:Description", data.payment.terms.as_deref());
        if let Some(due) = &data.due_date {
            xml += &format!(
                "<ram:DueDateDateTime>{}</ram:DueDateDateTime>",
                date("udt:DateTimeString", due)
            );
        }
        xml += "</ram:SpecifiedTradePaymentTerms>";
    }
    xml += "<ram:SpecifiedTradeSettlementHeaderMonetarySummation>";
    xml += &e("ram:LineTotalAmount", &invoice.totals.line_net);
    if invoice.charge_total != "0.00" {
        xml += &e("ram:ChargeTotalAmount", &invoice.charge_total);
    }
    if invoice.allowance_total != "0.00" {
        xml += &e("ram:AllowanceTotalAmount", &invoice.allowance_total);
    }
    xml += &e("ram:TaxBasisTotalAmount", &invoice.totals.tax_exclusive);
    xml += &element(
        "ram:TaxTotalAmount",
        Some(&invoice.totals.tax_total),
        &format!(" currencyID=\"{}\"", escape(&data.currency)),
    );
    xml += &e("ram:GrandTotalAmount", &invoice.totals.tax_inclusive);
    if invoice.prepaid != "0.00" {
        xml += &e("ram:TotalPrepaidAmount", &invoice.prepaid);
    }
    xml += &e("ram:DuePayableAmount", &invoice.totals.payable);
    xml += "</ram:SpecifiedTradeSettlementHeaderMonetarySummation>";
    for reference in preceding(data) {
        xml += "<ram:InvoiceReferencedDocument>";
        xml += &e("ram:IssuerAssignedID", &reference.invoice_number);
        if let Some(value) = &reference.issue_date {
            xml += &format!(
                "<ram:FormattedIssueDateTime>{}</ram:FormattedIssueDateTime>",
                date("qdt:DateTimeString", value)
            );
        }
        xml += "</ram:InvoiceReferencedDocument>";
    }
    xml += "</ram:ApplicableHeaderTradeSettlement></rsm:SupplyChainTradeTransaction></rsm:CrossIndustryInvoice>";
    Ok(xml)
}
