//! Offscreen Slint component verification; never opens or drives an OS window.
use slint::{
    ComponentHandle, Rgb8Pixel,
    platform::{
        Platform, PointerEventButton, WindowAdapter, WindowEvent,
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    },
};
use std::{cell::Cell, rc::Rc};
slint::include_modules!();
struct Offscreen(Rc<MinimalSoftwareWindow>);
impl Platform for Offscreen {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}
fn draw(window: &Rc<MinimalSoftwareWindow>, name: &str) -> Vec<Rgb8Pixel> {
    slint::platform::update_timers_and_animations();
    window.request_redraw();
    let mut pixels = vec![Rgb8Pixel::default(); 1320 * 760];
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, 1320);
    }));
    if let Some(path) = std::env::var_os("DRUCKWANDEL_UI_REVIEW_DIR") {
        let path = std::path::PathBuf::from(path);
        std::fs::create_dir_all(&path).unwrap();
        let mut encoder = png::Encoder::new(
            std::fs::File::create(path.join(format!("{name}.png"))).unwrap(),
            1320,
            760,
        );
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(
                &pixels
                    .iter()
                    .flat_map(|p| [p.r, p.g, p.b])
                    .collect::<Vec<_>>(),
            )
            .unwrap();
    }
    pixels
}
fn click(window: &MinimalSoftwareWindow, x: f32, y: f32) {
    let position = slint::LogicalPosition::new(x, y);
    window.dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    window.dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    slint::platform::update_timers_and_animations();
}
#[test]
fn settings_gear_and_pdf_preview_work_without_a_desktop_session() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Offscreen(window.clone()))).unwrap();
    let ui = MainWindow::new().unwrap();
    ui.show().unwrap();
    window.set_size(slint::PhysicalSize::new(1320, 760));
    ui.set_appearance(2);
    draw(&window, "start-dark");
    // The only icon button at the far right of the 1320 px toolbar.
    click(&window, 1280.0, 40.0);
    assert!(ui.get_settings_open(), "gear must open settings");
    draw(&window, "settings-dark");
    ui.set_appearance(1);
    draw(&window, "settings-light");
    click(&window, 1280.0, 40.0);
    assert!(!ui.get_settings_open());
    let bytes = std::env::var_os("DRUCKWANDEL_PREVIEW_PDF")
        .map(|path| std::fs::read(path).unwrap())
        .unwrap_or_else(|| include_bytes!("fixtures/source-invoice.pdf").to_vec());
    use sha2::{Digest, Sha256};
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let pdf = druckwandel_native::preview::PreviewDocument::from_bytes(bytes, &hash).unwrap();
    let requested = Rc::new(Cell::new(0));
    let count = requested.clone();
    ui.on_request_preview(move || count.set(count.get() + 1));
    ui.set_selected_name("Rechnung.pdf".into());
    ui.set_selected_path("local.pdf".into());
    ui.set_selected_hash(hash.into());
    let frame = pdf.render(0, 1000).unwrap();
    ui.set_preview_page_count(frame.page_count as i32);
    ui.set_preview_aspect(frame.height as f32 / frame.width as f32);
    ui.set_preview_image(slint::Image::from_rgba8_premultiplied(
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
            &frame.pixels,
            frame.width,
            frame.height,
        ),
    ));
    draw(&window, "preview-first-light");
    ui.set_appearance(2);
    draw(&window, "preview-first-dark");
    assert!(
        requested.get() > 0,
        "selecting a document must request its preview"
    );
    if frame.page_count > 1 {
        ui.set_preview_page(1);
        let frame = pdf.render(1, 1000).unwrap();
        ui.set_preview_aspect(frame.height as f32 / frame.width as f32);
        ui.set_preview_image(slint::Image::from_rgba8_premultiplied(
            slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                &frame.pixels,
                frame.width,
                frame.height,
            ),
        ));
        draw(&window, "preview-second-dark");
    }
    let before = requested.get();
    ui.set_preview_zoom(3);
    let before_scroll = draw(&window, "preview-zoom");
    window.dispatch_event(WindowEvent::PointerScrolled {
        position: slint::LogicalPosition::new(500.0, 400.0),
        delta_x: -160.0,
        delta_y: -250.0,
    });
    let after_scroll = draw(&window, "preview-zoom-scrolled");
    let changed = (250..630)
        .flat_map(|y| (300..730).map(move |x| y * 1320 + x))
        .filter(|i| before_scroll[*i] != after_scroll[*i])
        .count();
    assert!(
        changed > 1000,
        "zoomed page must pan inside its scroll viewport"
    );
    assert!(
        requested.get() > before,
        "zoom must request a sharper preview"
    );
    ui.hide().unwrap();
}
