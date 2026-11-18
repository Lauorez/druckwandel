//! Additional invoice fields and editable collection models for the review UI.
use crate::{AdjustmentData, MainWindow, OutputState, ReferenceData, optional};
use druckwandel_native::invoice::{
    AllowanceCharge, InvoiceInput, InvoiceLine, PrecedingInvoice, Tax,
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

fn text(value: &Option<String>) -> slint::SharedString {
    value.clone().unwrap_or_default().into()
}

fn adjustment_data(item: &AllowanceCharge) -> AdjustmentData {
    AdjustmentData {
        charge: item.charge,
        amount: item.amount.clone().into(),
        reason: text(&item.reason),
        reason_code: text(&item.reason_code),
        percent: text(&item.percent),
        base_amount: text(&item.base_amount),
        tax_category: item.tax.category_code.clone().into(),
        tax_rate: item.tax.rate.clone().into(),
        tax_reason: text(&item.tax.exemption_reason),
        tax_reason_code: text(&item.tax.exemption_reason_code),
    }
}

fn adjustments(model: ModelRc<AdjustmentData>) -> Vec<AllowanceCharge> {
    model
        .iter()
        .map(|item| AllowanceCharge {
            charge: item.charge,
            amount: item.amount.to_string(),
            reason: optional(item.reason.to_string()),
            reason_code: optional(item.reason_code.to_string()),
            percent: optional(item.percent.to_string()),
            base_amount: optional(item.base_amount.to_string()),
            tax: Tax {
                category_code: item.tax_category.to_string(),
                rate: item.tax_rate.to_string(),
                exemption_reason: optional(item.tax_reason.to_string()),
                exemption_reason_code: optional(item.tax_reason_code.to_string()),
            },
        })
        .collect()
}

pub fn set_fields(ui: &MainWindow, invoice: &InvoiceInput) {
    ui.set_invoice_kind(match invoice.invoice_type.as_str() {
        "381" => 1,
        "384" => 2,
        "326" => 3,
        _ => 0,
    });
    ui.set_invoice_phase(if invoice.final_invoice {
        2
    } else {
        i32::from(invoice.prepayment_invoice)
    });
    ui.set_service_date(text(&invoice.service_date));
    ui.set_currency(invoice.currency.clone().into());
    ui.set_prepaid_amount(text(&invoice.prepaid_amount));
    ui.set_payment_terms(text(&invoice.payment.terms));
    ui.set_payment_bic(text(&invoice.payment.bic));
    ui.set_payment_account(text(&invoice.payment.account_name));
    ui.set_payment_reference(text(&invoice.payment.payment_reference));
    ui.set_payment_means(invoice.payment.means_code.clone().into());
    ui.set_invoice_notes(invoice.notes.join("\n").into());
    ui.set_buyer_vat(text(&invoice.buyer.vat_id));
    ui.set_document_adjustments(ModelRc::new(VecModel::from(
        invoice
            .allowances
            .iter()
            .map(adjustment_data)
            .collect::<Vec<_>>(),
    )));
    let references = if invoice.preceding_invoices.is_empty() {
        invoice.preceding_invoice.iter().collect::<Vec<_>>()
    } else {
        invoice.preceding_invoices.iter().collect::<Vec<_>>()
    };
    ui.set_references(ModelRc::new(VecModel::from(
        references
            .into_iter()
            .map(|item| ReferenceData {
                number: item.invoice_number.clone().into(),
                date: text(&item.issue_date),
                paid: text(&item.paid_amount),
            })
            .collect::<Vec<_>>(),
    )));
}

pub fn collect_fields(ui: &MainWindow, invoice: &mut InvoiceInput) {
    invoice.invoice_type = ["380", "381", "384", "326"]
        .get(ui.get_invoice_kind() as usize)
        .unwrap_or(&"380")
        .to_string();
    invoice.prepayment_invoice = ui.get_invoice_phase() == 1;
    invoice.final_invoice = ui.get_invoice_phase() == 2;
    invoice.service_date = optional(ui.get_service_date().to_string());
    invoice.currency = ui.get_currency().trim().to_uppercase();
    invoice.prepaid_amount = optional(ui.get_prepaid_amount().to_string());
    invoice.payment.terms = optional(ui.get_payment_terms().to_string());
    invoice.payment.bic = optional(ui.get_payment_bic().to_string());
    invoice.payment.account_name = optional(ui.get_payment_account().to_string());
    invoice.payment.payment_reference = optional(ui.get_payment_reference().to_string());
    invoice.payment.means_code = ui.get_payment_means().trim().into();
    invoice.notes = ui
        .get_invoice_notes()
        .lines()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .collect();
    invoice.buyer.vat_id = optional(ui.get_buyer_vat().to_string());
    invoice.allowances = adjustments(ui.get_document_adjustments());
    invoice.preceding_invoices = ui
        .get_references()
        .iter()
        .map(|item| PrecedingInvoice {
            invoice_number: item.number.to_string(),
            issue_date: optional(item.date.to_string()),
            paid_amount: optional(item.paid.to_string()),
        })
        .collect();
    // Normalize the legacy single-reference field so deleting all rows is permanent.
    invoice.preceding_invoice = None;
}

pub fn set_line(ui: &MainWindow, line: Option<&InvoiceLine>) {
    ui.set_line_unit(line.map(|x| x.unit_code.clone()).unwrap_or_default().into());
    ui.set_line_description(line.map(|x| text(&x.description)).unwrap_or_default());
    ui.set_line_tax_category(
        line.map(|x| x.tax.category_code.clone())
            .unwrap_or_default()
            .into(),
    );
    ui.set_line_tax_reason(
        line.map(|x| text(&x.tax.exemption_reason))
            .unwrap_or_default(),
    );
    ui.set_line_tax_reason_code(
        line.map(|x| text(&x.tax.exemption_reason_code))
            .unwrap_or_default(),
    );
    ui.set_line_adjustments(ModelRc::new(VecModel::from(
        line.map(|x| x.allowances.iter().map(adjustment_data).collect::<Vec<_>>())
            .unwrap_or_default(),
    )));
}

pub fn collect_line(ui: &MainWindow, line: &mut InvoiceLine) {
    line.unit_code = ui.get_line_unit().trim().to_uppercase();
    line.description = optional(ui.get_line_description().to_string());
    line.tax.category_code = ui.get_line_tax_category().to_string();
    line.tax.exemption_reason = optional(ui.get_line_tax_reason().to_string());
    line.tax.exemption_reason_code = optional(ui.get_line_tax_reason_code().to_string());
    line.allowances = adjustments(ui.get_line_adjustments());
    for item in &mut line.allowances {
        item.tax = line.tax.clone();
    }
}

pub fn wire_collections(ui: &MainWindow) {
    let weak = ui.as_weak();
    ui.on_update_adjustment(move |line, index, item| {
        let Some(ui) = weak.upgrade() else { return };
        let model = if line {
            ui.get_line_adjustments()
        } else {
            ui.get_document_adjustments()
        };
        if index >= 0 && (index as usize) < model.row_count() {
            model.set_row_data(index as usize, item);
        }
        ui.global::<OutputState>().set_can_email(false);
    });
    let weak = ui.as_weak();
    ui.on_update_reference(move |index, item| {
        let Some(ui) = weak.upgrade() else { return };
        let model = ui.get_references();
        if index >= 0 && (index as usize) < model.row_count() {
            model.set_row_data(index as usize, item);
        }
        ui.global::<OutputState>().set_can_email(false);
    });
    let weak = ui.as_weak();
    ui.on_add_adjustment(move |line| {
        let Some(ui) = weak.upgrade() else { return };
        let model = if line {
            ui.get_line_adjustments()
        } else {
            ui.get_document_adjustments()
        };
        let mut items = model.iter().collect::<Vec<_>>();
        items.push(AdjustmentData {
            amount: "".into(),
            tax_category: ui.get_line_tax_category(),
            tax_rate: ui.get_line_tax_rate(),
            tax_reason: ui.get_line_tax_reason(),
            tax_reason_code: ui.get_line_tax_reason_code(),
            ..Default::default()
        });
        let model = ModelRc::new(VecModel::from(items));
        if line {
            ui.set_line_adjustments(model);
        } else {
            ui.set_document_adjustments(model);
        }
        ui.global::<OutputState>().set_can_email(false);
    });
    let weak = ui.as_weak();
    ui.on_remove_adjustment(move |line, index| {
        let Some(ui) = weak.upgrade() else { return };
        let model = if line {
            ui.get_line_adjustments()
        } else {
            ui.get_document_adjustments()
        };
        let mut items = model.iter().collect::<Vec<_>>();
        if index < 0 || index as usize >= items.len() {
            return;
        }
        items.remove(index as usize);
        let model = ModelRc::new(VecModel::from(items));
        if line {
            ui.set_line_adjustments(model);
        } else {
            ui.set_document_adjustments(model);
        }
        ui.global::<OutputState>().set_can_email(false);
    });
    let weak = ui.as_weak();
    ui.on_add_reference(move || {
        let Some(ui) = weak.upgrade() else { return };
        let mut items = ui.get_references().iter().collect::<Vec<_>>();
        items.push(ReferenceData::default());
        ui.set_references(ModelRc::new(VecModel::from(items)));
        ui.global::<OutputState>().set_can_email(false);
    });
    let weak = ui.as_weak();
    ui.on_remove_reference(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        let mut items = ui.get_references().iter().collect::<Vec<_>>();
        if index < 0 || index as usize >= items.len() {
            return;
        }
        items.remove(index as usize);
        ui.set_references(ModelRc::new(VecModel::from(items)));
        ui.global::<OutputState>().set_can_email(false);
    });
}
