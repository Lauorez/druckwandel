//! Local, single-page PDF preview. Rasterization never runs on the UI thread.
use hayro::{
    PixmapSettings, RenderCache, RenderSettings, hayro_interpret::InterpreterSettings,
    hayro_syntax::Pdf,
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::SystemTime,
};

const MAX_PDF_BYTES: u64 = 100 * 1024 * 1024;
const MAX_PIXELS: f32 = 4_000_000.0;

pub struct PreviewFrame {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub page_count: usize,
}

pub struct PreviewDocument(Pdf);
impl PreviewDocument {
    pub fn from_bytes(bytes: Vec<u8>, expected_hash: &str) -> Result<Self, String> {
        if bytes.len() as u64 > MAX_PDF_BYTES {
            return Err("PDF überschreitet 100 MB.".into());
        }
        if format!("{:x}", Sha256::digest(&bytes)) != expected_hash {
            return Err("Die Original-PDF wurde außerhalb der Anwendung verändert.".into());
        }
        Pdf::new(bytes)
            .map(Self)
            .map_err(|error| format!("PDF kann nicht angezeigt werden: {error:?}"))
    }
    pub fn render(&self, page_index: usize, requested_width: u32) -> Result<PreviewFrame, String> {
        let pages = self.0.pages();
        let page = pages
            .get(page_index)
            .ok_or("Diese PDF-Seite existiert nicht.")?;
        let (width, height) = page.render_dimensions();
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err("Ungültige PDF-Seitengröße.".into());
        }
        let scale = (requested_width.clamp(400, 2000) as f32 / width)
            .min(2800.0 / height)
            .min((MAX_PIXELS / (width * height)).sqrt());
        if !scale.is_finite() || scale <= 0.0 || width * scale < 1.0 || height * scale < 1.0 {
            return Err("PDF-Seite hat ein nicht unterstütztes Seitenverhältnis.".into());
        }
        let pixmap = hayro::render(
            page,
            &RenderCache::new(),
            &InterpreterSettings::default(),
            &RenderSettings::default(),
            &PixmapSettings {
                x_scale: scale,
                y_scale: scale,
                bg_color: hayro::vello_cpu::color::palette::css::WHITE,
            },
        );
        Ok(PreviewFrame {
            width: pixmap.width().into(),
            height: pixmap.height().into(),
            pixels: pixmap.data_as_u8_slice().to_vec(),
            page_count: pages.len(),
        })
    }
}

struct Request {
    generation: u64,
    path: PathBuf,
    hash: String,
    page: usize,
    width: u32,
}
struct CachedDocument {
    path: PathBuf,
    hash: String,
    modified: Option<SystemTime>,
    length: u64,
    document: PreviewDocument,
}
type Response = (u64, Result<PreviewFrame, String>);
#[derive(Default)]
struct Pending {
    request: Option<Request>,
    closed: bool,
}

/// One worker, one replaceable pending request and one replaceable response.
/// Rapid page changes cannot accumulate PDF buffers or rendering threads.
pub struct PreviewWorker {
    pending: Arc<(Mutex<Pending>, Condvar)>,
    response: Arc<Mutex<Option<Response>>>,
    generation: Arc<AtomicU64>,
}
impl PreviewWorker {
    pub fn new() -> Result<Self, String> {
        let worker = Self {
            pending: Arc::new((Mutex::new(Pending::default()), Condvar::new())),
            response: Arc::new(Mutex::new(None)),
            generation: Arc::new(AtomicU64::new(0)),
        };
        let (pending, response, generation) = (
            worker.pending.clone(),
            worker.response.clone(),
            worker.generation.clone(),
        );
        std::thread::Builder::new().name("pdf-preview".into()).spawn(move || {
            let mut cached: Option<CachedDocument> = None;
            loop {
                let request = {
                    let (lock, changed) = &*pending;
                    let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
                    while state.request.is_none() && !state.closed { state = changed.wait(state).unwrap_or_else(|e| e.into_inner()); }
                    if state.closed { break; }
                    state.request.take().expect("pending request")
                };
                let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render_request(&request, &mut cached)))
                    .unwrap_or_else(|_| { cached = None; Err("Diese Seite konnte nicht gerendert werden. Bitte im PDF-Programm öffnen.".into()) });
                if generation.load(Ordering::SeqCst) == request.generation {
                    *response.lock().unwrap_or_else(|e| e.into_inner()) = Some((request.generation, rendered));
                }
            }
        }).map_err(|e| format!("PDF-Vorschau konnte nicht gestartet werden: {e}"))?;
        Ok(worker)
    }
    pub fn request(&self, path: PathBuf, hash: String, page: usize, width: u32) {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let (lock, changed) = &*self.pending;
        lock.lock().unwrap_or_else(|e| e.into_inner()).request = Some(Request {
            generation,
            path,
            hash,
            page,
            width,
        });
        changed.notify_one();
    }
    pub fn take_response(&self) -> Option<Result<PreviewFrame, String>> {
        let response = self
            .response
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()?;
        (response.0 == self.generation.load(Ordering::SeqCst)).then_some(response.1)
    }
}
impl Drop for PreviewWorker {
    fn drop(&mut self) {
        let (lock, changed) = &*self.pending;
        let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
        state.closed = true;
        state.request = None;
        changed.notify_one();
    }
}
fn render_request(
    request: &Request,
    cached: &mut Option<CachedDocument>,
) -> Result<PreviewFrame, String> {
    let metadata = std::fs::metadata(&request.path).map_err(|e| e.to_string())?;
    if metadata.len() > MAX_PDF_BYTES {
        return Err("PDF überschreitet 100 MB.".into());
    }
    let modified = metadata.modified().ok();
    let hit = cached.as_ref().is_some_and(|c| {
        c.path == request.path
            && c.hash == request.hash
            && c.modified == modified
            && c.length == metadata.len()
    });
    if !hit {
        *cached = None;
        let mut bytes = Vec::new();
        File::open(&request.path)
            .map_err(|e| e.to_string())?
            .take(MAX_PDF_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let document = PreviewDocument::from_bytes(bytes, &request.hash)?;
        *cached = Some(CachedDocument {
            path: request.path.clone(),
            hash: request.hash.clone(),
            modified,
            length: metadata.len(),
            document,
        });
    }
    cached
        .as_ref()
        .expect("cached PDF")
        .document
        .render(request.page, request.width)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renders_invoice_ink_and_rejects_invalid_pages_and_changed_source() {
        let bytes = include_bytes!("../tests/fixtures/source-invoice.pdf").to_vec();
        let hash = format!("{:x}", Sha256::digest(&bytes));
        assert!(PreviewDocument::from_bytes(bytes.clone(), "wrong hash").is_err());
        let pdf = PreviewDocument::from_bytes(bytes, &hash).unwrap();
        assert!(pdf.render(1, 1000).is_err());
        let frame = pdf.render(0, 1000).unwrap();
        assert_eq!(frame.page_count, 1);
        assert_eq!(
            frame.pixels.len(),
            (frame.width * frame.height * 4) as usize
        );
        assert!(frame.width <= 1000 && frame.height <= 2800);
        assert!(
            frame
                .pixels
                .chunks_exact(4)
                .filter(|p| p[..3] != [255, 255, 255])
                .count()
                > 1000
        );
    }
    #[test]
    fn worker_returns_only_latest_request_and_rechecks_changed_files() {
        let bytes = include_bytes!("../tests/fixtures/source-invoice.pdf");
        let hash = format!("{:x}", Sha256::digest(bytes));
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source.pdf");
        std::fs::write(&path, bytes).unwrap();
        let worker = PreviewWorker::new().unwrap();
        for _ in 0..20 {
            worker.request(path.clone(), hash.clone(), 0, 1200);
        }
        worker.request(path.clone(), hash.clone(), 0, 400);
        fn wait(worker: &PreviewWorker) -> Result<PreviewFrame, String> {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            loop {
                if let Some(result) = worker.take_response() {
                    return result;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "preview worker timed out"
                );
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        assert!(wait(&worker).unwrap().width <= 400);
        std::fs::write(&path, b"externally changed source").unwrap();
        worker.request(path, hash, 0, 400);
        assert!(wait(&worker).is_err_and(|error| error.contains("verändert")));
    }
}
