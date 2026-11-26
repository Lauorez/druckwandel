use druckwandel_native::{
    extraction::read_document,
    file_io::sha256_hex,
    invoice::{InvoiceInput, calculate, validate, validate_en16931},
    recognition::{
        CONTRACT_VERSION, RecognitionMethod, RecognitionRegistry, RecognitionRequest, fill_missing,
        wsystem::{
            ExtractPayload, ExtractResponse, MAX_WSYSTEM_PDF_BYTES, WSYSTEM_BILLING_URL,
            WSystemClient, WSystemRecognizer, map_extract_response,
        },
    },
    secrets::{ApiKey, MemorySecretStore},
};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

struct CapturedRequest {
    method: String,
    target: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

struct MockReply {
    status: u16,
    reason: &'static str,
    headers: Vec<(&'static str, String)>,
    body: String,
}

fn spawn_mock_server(
    replies: Vec<MockReply>,
) -> (
    String,
    Arc<AtomicUsize>,
    thread::JoinHandle<Vec<CapturedRequest>>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_clone = Arc::clone(&hits);
    let handle = thread::spawn(move || {
        let mut captured = Vec::new();
        for reply in replies {
            let (mut stream, _) = listener.accept().unwrap();
            hits_clone.fetch_add(1, Ordering::SeqCst);
            let req = read_http_request(&mut stream);
            captured.push(req);
            let mut raw = format!(
                "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                reply.status,
                reply.reason,
                reply.body.len()
            );
            for (k, v) in reply.headers {
                raw.push_str(&format!("{k}: {v}\r\n"));
            }
            raw.push_str("\r\n");
            stream.write_all(raw.as_bytes()).unwrap();
            stream.write_all(reply.body.as_bytes()).unwrap();
            stream.flush().unwrap();
        }
        captured
    });
    (format!("http://{addr}"), hits, handle)
}

fn read_http_request(stream: &mut std::net::TcpStream) -> CapturedRequest {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end;
    loop {
        let n = stream.read(&mut chunk).unwrap();
        assert!(n > 0, "unexpected EOF before headers");
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
    }
    let header_text = String::from_utf8_lossy(&buf[..header_end - 4]).into_owned();
    let mut lines = header_text.lines();
    let request_line = lines.next().unwrap();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap().to_owned();
    let target = parts.next().unwrap().to_owned();
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_owned());
        }
    }
    let content_length = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk).unwrap();
        assert!(n > 0, "unexpected EOF reading body");
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);
    CapturedRequest {
        method,
        target,
        headers,
        body,
    }
}

fn fixture_request() -> (RecognitionRequest, Vec<u8>) {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/source-invoice.pdf");
    let bytes = std::fs::read(&path).unwrap();
    let sha = sha256_hex(&bytes);
    let request = RecognitionRequest {
        schema_version: CONTRACT_VERSION,
        document_sha256: sha,
        source_pdf: path.clone(),
        pages: read_document(&path).unwrap(),
        warnings: Vec::new(),
    };
    (request, bytes)
}

#[test]
fn extracts_structured_invoice_and_fields_and_preserves_manual_edits() {
    let (request, pdf_bytes) = fixture_request();
    let response_json = serde_json::json!({
        "status": "draft_for_review",
        "review_required": true,
        "format": "xrechnung",
        "invoice": {
            "format": "xrechnung",
            "invoice_number": "RE-2026-0001",
            "issue_date": "2026-08-20",
            "due_date": "2026-09-03",
            "delivery_date": "2026-08-18",
            "currency": "EUR",
            "buyer_reference": "04011000-12345-34",
            "seller": {
                "name": "Muster GmbH",
                "vat_id": "DE123456789",
                "tax_number": "201/113/40209",
                "street": "Hauptstr. 1",
                "city": "Berlin",
                "postal_code": "10115",
                "country": "DE",
                "email": "billing@muster.de",
                "phone": "+49 30 1234",
                "contact_name": "Anna Muster"
            },
            "buyer": {
                "name": "Kunde AG",
                "vat_id": "DE811193231",
                "street": "Ring 2",
                "city": "Muenchen",
                "postal_code": "80331",
                "country": "DE",
                "email": "ap@kunde.de"
            },
            "lines": [
                {
                    "description": "Beratung",
                    "quantity": 10,
                    "unit": "HUR",
                    "unit_price": 95.5,
                    "vat_rate": 19
                },
                {
                    "description": "Fachbuch",
                    "quantity": 2,
                    "unit_price": 12.99,
                    "vat_rate": 7
                }
            ],
            "payment": {
                "iban": "DE02 1203 0000 0000 2020 51",
                "bic": "BYLADEM1001",
                "terms": "14 Tage netto"
            }
        },
        "fields": [
            {"path": "invoice_number", "value": "RE-2026-0001", "confidence": 0.99, "checks": ["found_in_pdf_text"]},
            {"path": "payment.iban", "value": "DE02120300000000202051", "confidence": 0.98, "checks": ["found_in_pdf_text"]},
            {"path": "lines.0.unit_price_net", "value": 95.5, "confidence": 0.95, "checks": ["found_in_pdf_text"]}
        ],
        "arithmetic_check": {
            "all_match": true,
            "rows": [
                {"check": "sum of line nets (BT-106/109)", "computed": "980.98", "printed": "980.98", "match": true}
            ]
        },
        "validation": {"valid": true, "errors": []},
        "disclaimer": "AI-extracted draft for review."
    })
    .to_string();

    let (base_url, hits, server) = spawn_mock_server(vec![MockReply {
        status: 200,
        reason: "OK",
        headers: vec![
            ("Content-Type", "application/json".into()),
            ("X-Request-Id", "req_mock_extract_001".into()),
            ("X-Units-Used", "80".into()),
            ("X-Units-Remaining", "420".into()),
        ],
        body: response_json,
    }]);

    let secrets = Arc::new(MemorySecretStore::with_key(
        ApiKey::parse("wsk_test_mock_key_001").unwrap(),
    ));
    let client = WSystemClient::with_base_url(base_url).unwrap();
    let recognizer = WSystemRecognizer::new(client, secrets, true);
    let mut registry = RecognitionRegistry::default();
    registry.register(Box::new(recognizer)).unwrap();

    let result = registry
        .recognize(RecognitionMethod::WSystem, &request)
        .unwrap();
    let captured = server.join().unwrap();

    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert_eq!(captured[0].method, "POST");
    assert_eq!(captured[0].target, "/einvoice/extract?format=xrechnung");
    assert_eq!(
        captured[0].headers.get("x-api-key").map(String::as_str),
        Some("wsk_test_mock_key_001")
    );
    assert_eq!(
        captured[0].headers.get("content-type").map(String::as_str),
        Some("application/pdf")
    );
    assert!(captured[0].headers.contains_key("x-request-id"));
    assert!(captured[0].headers.contains_key("idempotency-key"));
    assert_eq!(captured[0].body, pdf_bytes);

    assert_eq!(result.method, RecognitionMethod::WSystem);
    assert_eq!(result.request_id.as_deref(), Some("req_mock_extract_001"));
    assert!(result.review_text().contains("req_mock_extract_001"));
    assert!(result.review_text().contains("W System Cloud"));
    assert_eq!(result.fields["invoiceNumber"].value, "RE-2026-0001");
    assert!(!result.fields["invoiceNumber"].source_token_ids.is_empty());
    assert_eq!(result.fields["iban"].value, "DE02120300000000202051");
    assert_eq!(result.fields["lineNet"].value, "980.98");
    assert_eq!(result.lines.len(), 2);
    assert_eq!(result.lines[0].line.name, "Beratung");
    assert_eq!(result.lines[0].line.unit_code, "HUR");
    assert_eq!(result.lines[1].line.unit_code, "C62");

    let draft = result.invoice_draft();
    assert!(
        validate(&draft).is_empty(),
        "unexpected validation errors: {:?}",
        validate(&draft)
    );
    let calculated = calculate(draft.clone()).unwrap();
    assert_eq!(calculated.totals.line_net, "980.98");

    let mut manual = draft;
    manual.invoice_number = "MANUELL-007".into();
    manual.seller.name = "Eigene Firma GmbH".into();
    fill_missing(&mut manual, &result);
    assert_eq!(manual.invoice_number, "MANUELL-007");
    assert_eq!(manual.seller.name, "Eigene Firma GmbH");
}

#[test]
fn maps_documented_needs_review_amounts_fields_only_response() {
    let (request, _) = fixture_request();
    // Exact shape published on https://wsystem.dev/pdf-to-xrechnung
    let payload: ExtractPayload = serde_json::from_str(
        r#"{
            "status": "needs_review_amounts",
            "review_required": true,
            "format": "xrechnung",
            "fields": [
                {"path": "invoice_number", "value": "RE-2026-0815", "confidence": 0.98, "checks": ["found_in_pdf_text"]},
                {"path": "issue_date", "value": "2026-08-20", "confidence": 0.96, "checks": ["found_in_pdf_text"]},
                {"path": "lines.1.description", "value": "Software-Wartung", "confidence": 0.92, "checks": ["found_in_pdf_text"]},
                {"path": "lines.1.quantity", "value": 2, "confidence": 0.90, "checks": ["found_in_pdf_text"]},
                {"path": "lines.1.unit_price_net", "value": 778, "confidence": 0.5, "checks": ["found_in_pdf_text", "totals_mismatch"]},
                {"path": "lines.1.vat_rate", "value": 19, "confidence": 0.91, "checks": ["found_in_pdf_text"]}
            ],
            "review": {
                "low_confidence": ["lines.1.unit_price_net"],
                "blocked": "extracted amounts do not match the printed totals; fix the lines, then POST the invoice JSON to /einvoice/create"
            },
            "arithmetic_check": {
                "all_match": false,
                "rows": [
                    {
                        "check": "sum of line nets (BT-106/109)",
                        "computed": "1556.00",
                        "printed": "778.00",
                        "match": false
                    }
                ]
            },
            "xml": null,
            "disclaimer": "AI-extracted draft for review. Fields can be wrong or missing even when the invoice validates; check every value against the source PDF."
        }"#,
    )
    .unwrap();

    let result = map_extract_response(
        &request,
        &ExtractResponse {
            payload,
            request_id: Some("req_arith_mismatch".into()),
            units_used: Some(80),
            units_remaining: Some(160),
            idempotent_replayed: false,
        },
    );

    assert_eq!(result.fields["invoiceNumber"].value, "RE-2026-0815");
    assert_eq!(result.fields["lineNet"].value, "778.00");
    assert_eq!(result.lines.len(), 1);
    assert_eq!(result.lines[0].line.id, "1");
    assert_eq!(result.lines[0].line.name, "Software-Wartung");
    assert_eq!(result.lines[0].line.quantity, "2");
    assert_eq!(result.lines[0].line.net_unit_price, "778");
    assert_eq!(result.lines[0].net_amount, "1556.00");
    assert!((result.lines[0].confidence - 0.5).abs() < f32::EPSILON);
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("needs_review_amounts"))
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("1556.00") && w.contains("778.00"))
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("niedriger Konfidenz") && w.contains("Nettopreis"))
    );
}

#[test]
fn maps_credit_note_small_business_and_allowances() {
    let (request, _) = fixture_request();
    let payload: ExtractPayload = serde_json::from_str(
        r#"{
            "status": "draft_for_review",
            "invoice": {
                "invoice_number": "CN-2026-001",
                "type_code": "381",
                "issue_date": "2026-10-25",
                "preceding_invoice": "INV-2026-001",
                "preceding_invoice_date": "2026-10-18",
                "currency": "EUR",
                "buyer_reference": "REF-1",
                "seller": {
                    "name": "Kleinunternehmen GmbH",
                    "tax_number": "201/113/40209",
                    "street": "Hauptstr. 1",
                    "postal_code": "10115",
                    "city": "Berlin",
                    "country": "DE"
                },
                "buyer": {
                    "name": "Kunde AG",
                    "street": "Ring 2",
                    "postal_code": "80331",
                    "city": "Muenchen",
                    "country": "DE"
                },
                "lines": [
                    {
                        "description": "Beratung",
                        "quantity": 2,
                        "unit": "HUR",
                        "unit_price": 100,
                        "vat_category": "E",
                        "vat_rate": 0
                    }
                ],
                "vat_exemption_reason": {
                    "E": "Kein Ausweis von Umsatzsteuer gemäß § 19 UStG"
                },
                "allowances": [
                    {
                        "amount": 10,
                        "reason": "Treuerabatt",
                        "vat_category": "E",
                        "vat_rate": 0
                    }
                ],
                "payment": {"iban": "DE89370400440532013000"}
            }
        }"#,
    )
    .unwrap();

    let result = map_extract_response(
        &request,
        &ExtractResponse {
            payload,
            request_id: Some("req_credit_sb".into()),
            units_used: None,
            units_remaining: None,
            idempotent_replayed: false,
        },
    );

    let draft = result.invoice_draft();
    assert_eq!(draft.invoice_type, "381");
    assert_eq!(draft.preceding_invoices.len(), 1);
    assert_eq!(draft.preceding_invoices[0].invoice_number, "INV-2026-001");
    assert_eq!(
        draft.preceding_invoices[0].issue_date.as_deref(),
        Some("2026-10-18")
    );
    assert_eq!(draft.lines[0].tax.category_code, "E");
    assert_eq!(
        draft.lines[0].tax.exemption_reason.as_deref(),
        Some("Kein Ausweis von Umsatzsteuer gemäß § 19 UStG")
    );
    assert_eq!(draft.allowances.len(), 1);
    assert!(!draft.allowances[0].charge);
    assert_eq!(draft.allowances[0].amount, "10.00");
    assert!(
        validate_en16931(&draft).is_empty(),
        "unexpected errors: {:?}",
        validate_en16931(&draft)
    );
    let calc = calculate(draft).unwrap();
    assert_eq!(calc.totals.tax_exclusive, "190.00");
    assert_eq!(calc.totals.payable, "190.00");
}

#[test]
fn falls_back_to_ubl_xml_when_fields_and_invoice_are_omitted() {
    let (request, _) = fixture_request();
    let sample: InvoiceInput =
        serde_json::from_str(include_str!("fixtures/export/standard.json")).unwrap();
    let ubl_xml = druckwandel_native::ubl::generate_ubl(sample.clone()).unwrap();

    let payload = ExtractPayload {
        status: Some("draft_for_review".into()),
        xml: Some(ubl_xml),
        ..Default::default()
    };
    let result = map_extract_response(
        &request,
        &ExtractResponse {
            payload,
            request_id: Some("req_xml_fallback".into()),
            units_used: None,
            units_remaining: None,
            idempotent_replayed: false,
        },
    );
    let draft = result.invoice_draft();
    assert_eq!(draft.invoice_number, sample.invoice_number);
    assert_eq!(draft.issue_date, sample.issue_date);
    assert_eq!(draft.seller.name, sample.seller.name);
    assert_eq!(draft.buyer.name, sample.buyer.name);
    assert_eq!(draft.lines.len(), sample.lines.len());
    assert!(
        validate(&draft).is_empty(),
        "UBL fallback should yield a valid draft: {:?}",
        validate(&draft)
    );
}

#[test]
fn translates_api_errors_with_request_id_and_never_leaks_api_key() {
    let (_, pdf_bytes) = fixture_request();
    let secret = "wsk_super_secret_do_not_leak";
    let key = ApiKey::parse(secret).unwrap();

    let cases: &[(u16, &'static str, &str, &str)] = &[
        (
            401,
            "Unauthorized",
            r#"{"error":"invalid_api_key","request_id":"req_401"}"#,
            "abgelehnt",
        ),
        (
            402,
            "Payment Required",
            r#"{"error":"paid_plan_required","request_id":"req_402"}"#,
            "Testkontingent",
        ),
        (
            409,
            "Conflict",
            r#"{"error":"idempotency_key_in_flight","request_id":"req_409"}"#,
            "läuft noch",
        ),
        (
            422,
            "Unprocessable Entity",
            r#"{"error":"no_text_layer","request_id":"req_422_scan"}"#,
            "Textebene",
        ),
        (
            422,
            "Unprocessable Entity",
            r#"{"error":"already_einvoice","request_id":"req_422_zug"}"#,
            "ZUGFeRD",
        ),
        (
            429,
            "Too Many Requests",
            r#"{"error":"monthly_quota_exhausted","request_id":"req_429"}"#,
            "Kontingent",
        ),
        (
            501,
            "Not Implemented",
            r#"{"error":"extract_not_enabled","request_id":"req_501"}"#,
            "deaktiviert",
        ),
        (
            302,
            "Found",
            r#"{"error":"redirect","request_id":"req_302"}"#,
            "302",
        ),
    ];

    let replies = cases
        .iter()
        .map(|(status, reason, body, _)| MockReply {
            status: *status,
            reason,
            headers: vec![
                ("Content-Type", "application/json".into()),
                ("Location", "https://evil.example/steal".into()),
            ],
            body: (*body).to_owned(),
        })
        .collect();

    let (base_url, _, server) = spawn_mock_server(replies);
    let client = WSystemClient::with_base_url(base_url).unwrap();

    for (_, _, body, expected_fragment) in cases {
        let err = client.extract_pdf(&key, &pdf_bytes).unwrap_err();
        assert!(
            err.contains(expected_fragment),
            "expected '{expected_fragment}' in '{err}'"
        );
        let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
        let req_id = parsed["request_id"].as_str().unwrap();
        assert!(err.contains(req_id), "expected {req_id} in '{err}'");
        assert!(!err.contains(secret), "secret leaked in error: '{err}'");
    }
    let _ = server.join().unwrap();
}

#[test]
fn checks_billing_usage_and_supports_header_fallback() {
    let key = ApiKey::parse("wsk_usage_key").unwrap();
    let (base_url, _, server) = spawn_mock_server(vec![
        MockReply {
            status: 200,
            reason: "OK",
            headers: vec![
                ("Content-Type", "application/json".into()),
                ("X-Request-Id", "req_usage_1".into()),
            ],
            body: r#"{"plan":"free","quota":500,"remaining":269}"#.into(),
        },
        MockReply {
            status: 200,
            reason: "OK",
            headers: vec![
                ("Content-Type", "application/json".into()),
                ("X-Request-Id", "req_usage_2".into()),
                ("X-Units-Used", "80".into()),
                ("X-Units-Remaining", "24920".into()),
            ],
            body: r#"{"plan":"pro"}"#.into(),
        },
    ]);
    let client = WSystemClient::with_base_url(base_url).unwrap();

    let first = client.check_usage(&key).unwrap();
    assert_eq!(first.plan.as_deref(), Some("free"));
    assert_eq!(first.quota, Some(500));
    assert_eq!(first.remaining, Some(269));
    assert_eq!(first.request_id.as_deref(), Some("req_usage_1"));
    assert!(first.summary().contains("269 von 500"));

    let second = client.check_usage(&key).unwrap();
    assert_eq!(second.plan.as_deref(), Some("pro"));
    assert_eq!(second.used, Some(80));
    assert_eq!(second.remaining, Some(24920));
    assert!(second.summary().contains("24920 Einheiten"));

    let captured = server.join().unwrap();
    assert_eq!(captured[0].method, "GET");
    assert_eq!(captured[0].target, "/billing/usage");
    assert_eq!(
        captured[0].headers.get("x-api-key").map(String::as_str),
        Some("wsk_usage_key")
    );
}

#[test]
fn guards_reject_missing_consent_missing_key_tampered_pdf_and_oversized_pdf() {
    let (mut request, _) = fixture_request();
    let client = WSystemClient::with_base_url("http://127.0.0.1:9").unwrap();
    let secrets = Arc::new(MemorySecretStore::with_key(
        ApiKey::parse("wsk_guard_key").unwrap(),
    ));

    // Missing consent
    let mut reg_no_consent = RecognitionRegistry::default();
    reg_no_consent
        .register(Box::new(WSystemRecognizer::new(
            client.clone(),
            Arc::clone(&secrets) as Arc<dyn druckwandel_native::secrets::SecretStore>,
            false,
        )))
        .unwrap();
    assert!(
        reg_no_consent
            .recognize(RecognitionMethod::WSystem, &request)
            .unwrap_err()
            .contains("nicht freigegeben")
    );

    // Missing API key
    let mut reg_no_key = RecognitionRegistry::default();
    reg_no_key
        .register(Box::new(WSystemRecognizer::new(
            client.clone(),
            Arc::new(MemorySecretStore::default()),
            true,
        )))
        .unwrap();
    assert!(
        reg_no_key
            .recognize(RecognitionMethod::WSystem, &request)
            .unwrap_err()
            .contains("Kein W-System-API-Schlüssel")
    );

    // Tampered PDF (SHA-256 mismatch)
    let mut reg_tampered = RecognitionRegistry::default();
    reg_tampered
        .register(Box::new(WSystemRecognizer::new(
            client.clone(),
            secrets,
            true,
        )))
        .unwrap();
    request.document_sha256 = "0".repeat(64);
    assert!(
        reg_tampered
            .recognize(RecognitionMethod::WSystem, &request)
            .unwrap_err()
            .contains("verändert")
    );

    // Oversized PDF (> 10 MB)
    let mut oversized = b"%PDF-1.7\n".to_vec();
    oversized.resize((MAX_WSYSTEM_PDF_BYTES as usize) + 1, b'0');
    let err = client
        .extract_pdf(&ApiKey::parse("wsk_guard_key").unwrap(), &oversized)
        .unwrap_err();
    assert!(err.contains("10 MB"));
}

#[test]
fn maps_live_verified_extract_and_usage_contract() {
    assert_eq!(
        WSYSTEM_BILLING_URL,
        "https://api.wsystem.dev/billing/druckwandel"
    );

    let (request, _) = fixture_request();
    // Exact sanitized response shape verified against live POST /einvoice/extract?format=xrechnung
    let payload: ExtractPayload = serde_json::from_str(
        r#"{
            "status": "draft_for_review",
            "review_required": true,
            "format": "xrechnung",
            "pages": 1,
            "document_kind": "invoice",
            "invoice": {
                "format": "xrechnung",
                "invoice_number": "RE-2026-0001",
                "issue_date": "2026-08-20",
                "currency": "EUR",
                "seller": {
                    "name": "Muster Elektro GmbH",
                    "vat_id": "DE123456789",
                    "street": "Werkstraße 1",
                    "city": "Berlin",
                    "postal_code": "10115",
                    "country": "DE",
                    "email": "rechnung@muster.invalid",
                    "phone": "+49 30 123456",
                    "contact_name": "Erika Muster"
                },
                "buyer": {
                    "name": "Beispiel Bau AG",
                    "street": "Kundenweg 9",
                    "city": "Hamburg",
                    "postal_code": "20095",
                    "country": "DE"
                },
                "lines": [
                    {
                        "description": "Montagestunde",
                        "quantity": 2.5,
                        "unit": "HUR",
                        "unit_price": 80.0,
                        "vat_category": "S",
                        "vat_rate": 19.0
                    },
                    {
                        "description": "Kabel",
                        "quantity": 3.0,
                        "unit": "MTR",
                        "unit_price": 12.35,
                        "vat_category": "S",
                        "vat_rate": 19.0
                    }
                ],
                "due_date": "2026-09-03",
                "delivery_date": "2026-08-19",
                "buyer_reference": "04011000-12345-03",
                "payment": {
                    "iban": "DE02120300000000202051",
                    "means_code": "58",
                    "terms": "Zahlbar ohne Abzug innerhalb von 14 Tagen."
                }
            },
            "vat_breakdown": [
                {
                    "vat_category": "S",
                    "vat_rate": 19.0,
                    "taxable_amount": 237.04,
                    "tax_amount": 45.04,
                    "exemption_reason_text": null,
                    "exemption_reason_code": null
                }
            ],
            "fields": [
                {"path": "invoice_number", "value": "RE-2026-0001", "confidence": 0.95, "checks": ["found_in_pdf_text"]},
                {"path": "seller.vat_id", "value": "DE123456789", "confidence": 0.95, "checks": ["found_in_pdf_text", "vat_id_format_ok"]},
                {"path": "lines.0.description", "value": "Montagestunde", "confidence": 0.95, "checks": ["found_in_pdf_text"]},
                {"path": "lines.0.quantity", "value": 2.5, "confidence": 0.9, "checks": ["totals_match_printed"]},
                {"path": "lines.1.unit_price", "value": 12.35, "confidence": 0.95, "checks": ["found_in_pdf_text", "totals_match_printed"]},
                {"path": "payment.iban", "value": "DE02120300000000202051", "confidence": 0.95, "checks": ["found_in_pdf_text", "iban_checksum_ok"]}
            ],
            "review": {
                "low_confidence": [],
                "min_confidence": 0.85,
                "model_uncertainties": ["Unit price of line 2 may have 3 decimal places"],
                "input_error": null,
                "blocked": null
            },
            "arithmetic_check": {
                "all_match": true,
                "rows": [
                    {"check": "lines.0: quantity x unit price = line net", "computed": "200.00", "printed": "200.0", "match": true},
                    {"check": "lines.1: quantity x unit price = line net", "computed": "37.05", "printed": "37.04", "match": true},
                    {"check": "sum of line nets (BT-106/109)", "computed": "237.05", "printed": "237.04", "match": true},
                    {"check": "VAT total (BT-110)", "computed": "45.04", "printed": "45.04", "match": true},
                    {"check": "total with VAT (BT-112)", "computed": "282.09", "printed": "282.08", "match": true},
                    {"check": "amount due (BT-115)", "computed": "282.09", "printed": "282.08", "match": true}
                ],
                "note": "1-cent rounding difference tolerated"
            },
            "totals": {
                "currency": "EUR",
                "line_total": "237.05",
                "vat_total": "45.04",
                "total_with_vat": "282.09",
                "payable": "282.09"
            },
            "validation": {
                "valid": false,
                "errors": [
                    {
                        "rule_id": "PEPPOL-EN16931-R010",
                        "message": "Buyer electronic address MUST be provided"
                    }
                ]
            },
            "xml": "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Invoice xmlns=\"urn:oasis:names:specification:ubl:schema:xsd:Invoice-2\" xmlns:cac=\"urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2\" xmlns:cbc=\"urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2\"><cac:LegalMonetaryTotal><cbc:LineExtensionAmount currencyID=\"EUR\">237.05</cbc:LineExtensionAmount><cbc:TaxInclusiveAmount currencyID=\"EUR\">282.09</cbc:TaxInclusiveAmount><cbc:PayableAmount currencyID=\"EUR\">282.09</cbc:PayableAmount></cac:LegalMonetaryTotal></Invoice>",
            "disclaimer": "AI-extracted draft for review."
        }"#,
    )
    .unwrap();

    let result = map_extract_response(
        &request,
        &ExtractResponse {
            payload,
            request_id: Some("dw-live-shape-1".into()),
            units_used: Some(80),
            units_remaining: Some(7920),
            idempotent_replayed: false,
        },
    );

    // Document-level printed totals must win over per-line arithmetic rows (200.00) and computed XML (237.05 / 282.09).
    assert_eq!(result.fields["lineNet"].value, "237.04");
    assert_eq!(result.fields["taxTotal"].value, "45.04");
    assert_eq!(result.fields["taxInclusive"].value, "282.08");
    assert_eq!(result.fields["payable"].value, "282.08");
    assert!(
        result.fields["sellerVatId"]
            .source_text
            .contains("USt-IdNr.-Format gültig")
    );
    assert!(
        result.fields["iban"]
            .source_text
            .contains("IBAN-Prüfsumme gültig")
    );
    assert_eq!(result.lines.len(), 2);
    assert_eq!(result.lines[0].line.name, "Montagestunde");
    assert_eq!(result.lines[1].line.name, "Kabel");
    assert!(
        result.lines[0]
            .source_text
            .contains("mit gedruckten Summen abgeglichen")
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("Unsicherheiten") && w.contains("3 decimal places"))
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("1-cent rounding difference tolerated"))
    );

    // Verify live /billing/usage payload shape including period and unit_weights.
    let key = ApiKey::parse("wsk_live_shape_key").unwrap();
    let (base_url, _, server) = spawn_mock_server(vec![MockReply {
        status: 200,
        reason: "OK",
        headers: vec![
            ("Content-Type", "application/json".into()),
            ("X-Request-ID", "dw-live-usage-1".into()),
        ],
        body: r#"{"plan":"partner","quota":8000,"used":80,"remaining":7920,"period":"2026-11","unit_weights":{"/einvoice/extract":80}}"#.into(),
    }]);
    let client = WSystemClient::with_base_url(base_url).unwrap();
    let usage = client.check_usage(&key).unwrap();
    assert_eq!(usage.plan.as_deref(), Some("partner"));
    assert_eq!(usage.period.as_deref(), Some("2026-11"));
    assert_eq!(usage.extract_unit_weight, Some(80));
    assert_eq!(usage.remaining, Some(7920));
    let summary = usage.summary();
    assert!(summary.contains("Tarif: partner"));
    assert!(summary.contains("Zeitraum: 2026-11"));
    assert!(summary.contains("7920 von 8000 Einheiten"));
    assert!(summary.contains("ca. 99 Rechnungen à 80 Einheiten"));
    let _ = server.join().unwrap();
}
