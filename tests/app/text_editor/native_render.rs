use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::{ComponentHandle, Rgb8Pixel};
use std::rc::Rc;

slint::slint! {
    export component EditorRenderTest inherits Window {
        width: 360px; height: 240px;
        background: #000000;
        in-out property <string> content;
        in property <string> source;
        in property <string> spans;
        in property <length> text-width: 340px;
        in property <length> gutter-width: 0px;
        in property <length> scroll-y: 0px;
        out property <length> measured-height: input.preferred-height;
        out property <length> cursor-x;
        out property <length> cursor-y;
        public function select(start: int, end: int) {
            input.set-selection-offsets(start, end);
        }
        public function focus-editor() { input.focus(); }
        input := TextInput {
            x: 8px; y: 8px + root.scroll-y;
            width: root.text-width; height: 224px;
            font-family: "Consolas"; font-size: 18px;
            color: #ffffff;
            selection-foreground-color: #ffffff;
            selection-background-color: #224488;
            single-line: false;
            wrap: char-wrap;
            text <=> root.content;
            syntax-source: root.source;
            syntax-spans: root.spans;
            line-number-width: root.gutter-width;
            cursor-position-changed(position) => {
                root.cursor-x = position.x;
                root.cursor-y = position.y;
            }
        }
    }
}

struct Backend(Rc<MinimalSoftwareWindow>);
impl slint::platform::Platform for Backend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

#[test]
fn editor_native_render_preserves_geometry_selection_and_editing() {
    // Slint contexts are thread-local; avoid interacting with any app window.
    std::thread::spawn(|| {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        slint::platform::set_platform(Box::new(Backend(window.clone()))).unwrap();
        let ui = EditorRenderTest::new().unwrap();
        ui.show().unwrap();
        window.set_size(slint::PhysicalSize::new(360, 240));
        let render = || {
            let mut pixels = vec![Rgb8Pixel::default(); 360 * 240];
            window.request_redraw();
            assert!(window.draw_if_needed(|renderer| {
                renderer.render(&mut pixels, 360);
            }));
            pixels
        };
        let text = "let 名称 = \"if 中文\"; // return\n\nfn main() {\n    let value = 12345;\n}\n";
        ui.set_content(text.into());
        ui.set_source(text.into());
        for width in [340.0, 140.0] {
            ui.set_text_width(width);
            ui.set_spans("".into());
            ui.invoke_select(0, 0);
            let plain = render();
            let height = ui.get_measured_height();
            ui.set_spans(super::super::editor_syntax::highlight(text, "test.rs").into());
            let colored = render();
            assert_eq!(height, ui.get_measured_height());
            assert!(
                colored.iter().any(|p| p.r != p.g || p.g != p.b),
                "syntax colors missing"
            );
            let shape = |pixels: &[Rgb8Pixel]| {
                pixels
                    .iter()
                    .map(|p| p.r > 0 || p.g > 0 || p.b > 0)
                    .collect::<Vec<_>>()
            };
            let plain_shape = shape(&plain);
            let color_shape = shape(&colored);
            let different = plain_shape
                .iter()
                .zip(&color_shape)
                .filter(|(a, b)| a != b)
                .count();
            let occupied = plain_shape.iter().filter(|&&v| v).count();
            eprintln!("width={width}: mask differences={different}, occupied={occupied}");
            for (name, pixels) in [("plain", &plain), ("color", &colored)] {
                let bytes: Vec<u8> = pixels.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("target/editor-{name}-{width}.png"));
                image::save_buffer(path, &bytes, 360, 240, image::ColorType::Rgb8).unwrap();
            }
            // Quantizing alpha against different foreground colors can lose
            // a few faint antialiasing pixels; it must not move glyphs.
            assert!(
                different <= occupied / 50 + 4,
                "highlight changed glyph geometry"
            );

            ui.invoke_focus_editor();
            ui.invoke_select(0, text.len() as i32);
            let selected_color = render();
            let cursor = (ui.get_cursor_x(), ui.get_cursor_y());
            ui.set_spans("".into());
            let selected_plain = render();
            assert!(
                selected_color == selected_plain,
                "syntax overrode selection colors"
            );
            assert_eq!(cursor, (ui.get_cursor_x(), ui.get_cursor_y()));
            ui.invoke_select(0, 0);
        }
        ui.set_text_width(340.0);
        ui.set_spans(super::super::editor_syntax::highlight(text, "test.rs").into());
        let screenshot = render();
        let bytes: Vec<u8> = screenshot.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/editor-syntax-qa.png");
        image::save_buffer(path, &bytes, 360, 240, image::ColorType::Rgb8).unwrap();

        // Large-file layouts have two native items in the app rather than
        // thousands of hidden TextInputs. Measure initial paint vs cached paint.
        let large = "let value = 123; // comment\n".repeat(9_999);
        ui.set_content(large.clone().into());
        ui.set_source(large.clone().into());
        ui.set_spans(super::super::editor_syntax::highlight(&large, "large.rs").into());
        let first_at = std::time::Instant::now();
        render();
        let first_time = first_at.elapsed();
        let cached_at = std::time::Instant::now();
        render();
        let cached_time = cached_at.elapsed();
        eprintln!("editor 10,000 lines: first paint {first_time:?}, cached paint {cached_time:?}");
        ui.set_gutter_width(42.0);
        ui.set_spans("".into());
        ui.set_scroll_y(-36.0);
        let gutter = render();
        assert!(gutter.iter().any(|p| p.r > 0), "gutter missing");
        assert!(
            gutter
                .chunks(360)
                .all(|row| row[52..].iter().all(|p| p.r == 0)),
            "gutter painted document text instead of numbers"
        );
        ui.set_gutter_width(0.0);
        ui.set_scroll_y(0.0);
        ui.set_content(text.into());
        ui.set_source(text.into());
        ui.set_spans(super::super::editor_syntax::highlight(text, "test.rs").into());

        // Stale syntax must not apply to newly typed text.
        ui.invoke_select(0, text.len() as i32);
        window.dispatch_event(slint::platform::WindowEvent::KeyPressed { text: "x".into() });
        window.dispatch_event(slint::platform::WindowEvent::KeyReleased { text: "x".into() });
        assert_eq!(ui.get_content().as_str(), "x");
        let stale = render();
        ui.set_spans("".into());
        assert!(stale == render());
    })
    .join()
    .unwrap();
}

#[test]
fn editor_app_window_renders_highlighted_document() {
    std::thread::spawn(|| {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        slint::platform::set_platform(Box::new(Backend(window.clone()))).unwrap();
        let ui = super::super::EditorWindow::new().unwrap();
        let text = "// Unicode and wrapping\nfn main() {\n    let message = \"Hello, 中文\";\n    println!(\"{}\", message);\n}\n";
        ui.set_editor_path("example.rs".into());
        ui.set_editor_name("example.rs".into());
        ui.set_editor_content(text.into());
        super::super::editor_syntax::refresh(&ui, text);
        ui.set_editor_open(true);
        ui.show().unwrap();
        window.set_size(slint::PhysicalSize::new(1000, 700));
        let mut pixels = vec![Rgb8Pixel::default(); 1000 * 700];
        window.request_redraw();
        assert!(window.draw_if_needed(|renderer| { renderer.render(&mut pixels, 1000); }));
        assert_eq!(ui.get_editor_language().as_str(), "Rust");
        assert!(!ui.get_editor_syntax_spans().is_empty());
        let bytes: Vec<u8> = pixels.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/editor-window-qa.png");
        image::save_buffer(path, &bytes, 1000, 700, image::ColorType::Rgb8).unwrap();
    }).join().unwrap();
}
