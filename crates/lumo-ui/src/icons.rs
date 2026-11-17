//! Vector icons, drawn rather than taken from fonts so they look the same on
//! every machine and can never render as a missing glyph.
//!
//! Two families, both on a 16×16 grid:
//! - **Line icons** (actions, places): 1.5px strokes in the caller's color,
//!   in the spirit of Zed's / Lucide's icon set.
//! - **File-type icons**: each type has its own *shape and* color, so types
//!   are told apart at a glance even by color-blind users.

use std::f32::consts::{FRAC_PI_2, PI};

use eframe::egui::{
    Color32, Painter, Pos2, Rect, Response, Role, Sense, Shape, Stroke, Ui, Vec2, WidgetInfo, vec2,
};

use crate::theme::Palette;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    // Line icons: actions.
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    Refresh,
    Plus,
    Close,
    ChevronRight,
    SortUp,
    SortDown,
    Eye,
    EyeOff,
    // Line icons: places.
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
    Drive,
    // Filled, colored icons for list entries.
    Folder,
    File(FileKind),
    Link,
}

/// Broad file categories, each with a distinct icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Generic,
    Code,
    Text,
    Pdf,
    Sheet,
    Image,
    Video,
    Audio,
    Archive,
    Executable,
    Config,
}

impl FileKind {
    pub fn from_name(name: &str) -> Self {
        let ext = std::path::Path::new(name)
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase());
        match ext.as_deref() {
            Some(
                "rs" | "py" | "js" | "mjs" | "ts" | "tsx" | "jsx" | "c" | "h" | "cpp" | "hpp"
                | "cs" | "go" | "java" | "kt" | "swift" | "rb" | "php" | "html" | "css" | "scss"
                | "sh" | "ps1" | "sql" | "lua" | "zig" | "dart" | "vue" | "svelte",
            ) => Self::Code,
            Some("txt" | "md" | "rtf" | "doc" | "docx" | "odt" | "log" | "tex") => Self::Text,
            Some("pdf") => Self::Pdf,
            Some("xls" | "xlsx" | "ods" | "csv" | "tsv") => Self::Sheet,
            Some(
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" | "heic" | "tif"
                | "tiff" | "avif" | "psd",
            ) => Self::Image,
            Some("mp4" | "mkv" | "avi" | "mov" | "webm" | "wmv" | "m4v") => Self::Video,
            Some("mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus") => Self::Audio,
            Some("zip" | "rar" | "7z" | "tar" | "gz" | "tgz" | "xz" | "bz2" | "zst" | "iso") => {
                Self::Archive
            }
            Some("exe" | "msi" | "bat" | "cmd" | "dll" | "appx" | "msix" | "com") => {
                Self::Executable
            }
            Some(
                "json" | "toml" | "yaml" | "yml" | "xml" | "ini" | "cfg" | "conf" | "env" | "lock",
            ) => Self::Config,
            _ => Self::Generic,
        }
    }

    fn color(self) -> Color32 {
        let (r, g, b) = match self {
            Self::Generic => (0x9A, 0xA0, 0xA8),
            Self::Code => (0x2E, 0xB8, 0xC9),
            Self::Text => (0x5B, 0x8D, 0xEF),
            Self::Pdf => (0xE5, 0x48, 0x4D),
            Self::Sheet => (0x2E, 0xA0, 0x5B),
            Self::Image => (0x9B, 0xC5, 0x3D),
            Self::Video => (0xA0, 0x6C, 0xF0),
            Self::Audio => (0xEC, 0x5F, 0xA8),
            Self::Archive => (0xC9, 0x9A, 0x3B),
            Self::Executable => (0x6C, 0x8E, 0xBF),
            Self::Config => (0x8F, 0x9B, 0xAE),
        };
        Color32::from_rgb(r, g, b)
    }
}

const FOLDER_BACK: Color32 = Color32::from_rgb(0xC9, 0x8A, 0x2E);
const FOLDER_FRONT: Color32 = Color32::from_rgb(0xE8, 0xA8, 0x45);

/// Paints `icon` into `rect`. Line icons use `color`; file icons use their own.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let g = Grid::new(painter, rect, color);
    match icon {
        Icon::ArrowLeft => g.arrow((13.0, 8.0), (3.0, 8.0)),
        Icon::ArrowRight => g.arrow((3.0, 8.0), (13.0, 8.0)),
        Icon::ArrowUp => g.arrow((8.0, 13.5), (8.0, 2.5)),
        Icon::Refresh => {
            // 300° arc, open at the top right, with an arrowhead on its end.
            let start = -FRAC_PI_2 + 0.5;
            let points = g.arc((8.0, 8.5), 5.0, start, start + 5.3);
            let tip = points[points.len() - 1];
            let before = points[points.len() - 2];
            g.polyline(points);
            g.head(before, tip);
        }
        Icon::Plus => {
            g.line((8.0, 3.0), (8.0, 13.0));
            g.line((3.0, 8.0), (13.0, 8.0));
        }
        Icon::Close => {
            g.line((4.0, 4.0), (12.0, 12.0));
            g.line((12.0, 4.0), (4.0, 12.0));
        }
        Icon::ChevronRight => g.polyline_at(&[(6.0, 3.5), (10.5, 8.0), (6.0, 12.5)]),
        Icon::SortUp => g.polyline_at(&[(4.0, 10.0), (8.0, 6.0), (12.0, 10.0)]),
        Icon::SortDown => g.polyline_at(&[(4.0, 6.0), (8.0, 10.0), (12.0, 6.0)]),
        Icon::Eye | Icon::EyeOff => {
            g.polyline(g.curve((1.5, 8.0), (8.0, 2.0), (14.5, 8.0)));
            g.polyline(g.curve((1.5, 8.0), (8.0, 14.0), (14.5, 8.0)));
            g.circle((8.0, 8.0), 2.2);
            if icon == Icon::EyeOff {
                g.line((2.5, 2.5), (13.5, 13.5));
            }
        }
        Icon::Home => {
            g.polyline_at(&[(2.0, 8.0), (8.0, 2.5), (14.0, 8.0)]);
            g.polyline_at(&[(3.8, 6.5), (3.8, 13.5), (12.2, 13.5), (12.2, 6.5)]);
            g.polyline_at(&[(6.8, 13.5), (6.8, 9.8), (9.2, 9.8), (9.2, 13.5)]);
        }
        Icon::Desktop => {
            g.rect((1.5, 2.5), (14.5, 11.0), 1.5);
            g.line((8.0, 11.0), (8.0, 13.5));
            g.line((5.0, 13.5), (11.0, 13.5));
        }
        Icon::Documents => {
            g.page_outline();
            g.line((5.5, 8.0), (10.5, 8.0));
            g.line((5.5, 10.5), (10.5, 10.5));
            g.line((5.5, 13.0), (8.5, 13.0));
        }
        Icon::Downloads => {
            g.arrow((8.0, 2.0), (8.0, 10.0));
            g.polyline_at(&[(2.0, 10.0), (2.0, 14.0), (14.0, 14.0), (14.0, 10.0)]);
        }
        Icon::Pictures => {
            g.rect((1.5, 2.5), (14.5, 13.5), 1.5);
            g.polyline_at(&[
                (1.5, 12.0),
                (6.0, 7.5),
                (9.5, 11.0),
                (11.5, 9.0),
                (14.5, 12.0),
            ]);
            g.circle((10.8, 5.8), 1.2);
        }
        Icon::Music => {
            g.line((6.0, 12.0), (6.0, 3.5));
            g.line((13.0, 11.0), (13.0, 2.5));
            g.line((6.0, 3.5), (13.0, 2.5));
            g.dot((4.3, 12.0), 1.9);
            g.dot((11.3, 11.0), 1.9);
        }
        Icon::Videos => {
            g.rect((1.5, 3.0), (14.5, 13.0), 1.5);
            g.polyline_at(&[(6.5, 5.8), (10.8, 8.0), (6.5, 10.2), (6.5, 5.8)]);
        }
        Icon::Drive => {
            g.rect((1.5, 4.5), (14.5, 11.5), 1.5);
            g.line((1.5, 8.5), (14.5, 8.5));
            g.dot((11.8, 10.0), 0.8);
        }
        Icon::Folder => g.folder(),
        Icon::File(kind) => g.file(kind),
        Icon::Link => {
            g.file(FileKind::Generic);
            let badge = g.rect_at((0.5, 8.5), (7.5, 15.5));
            painter.rect_filled(badge, 1.5 * g.unit, Color32::from_rgb(0x3B, 0x82, 0xF6));
            let white = Stroke::new(1.3 * g.unit, Color32::WHITE);
            for (a, b) in [
                ((2.5, 13.5), (5.5, 10.5)),
                ((3.5, 10.5), (5.5, 10.5)),
                ((5.5, 10.5), (5.5, 12.5)),
            ] {
                painter.line_segment([g.at(a), g.at(b)], white);
            }
        }
    }
}

/// Drawing helpers on a 16×16 design grid scaled into a rect.
#[derive(Clone, Copy)]
struct Grid<'a> {
    painter: &'a Painter,
    origin: Pos2,
    unit: f32,
    stroke: Stroke,
}

impl<'a> Grid<'a> {
    fn new(painter: &'a Painter, rect: Rect, color: Color32) -> Self {
        let unit = rect.width() / 16.0;
        Self {
            painter,
            origin: rect.min,
            unit,
            stroke: Stroke::new(1.5 * unit, color),
        }
    }

    fn at(&self, (x, y): (f32, f32)) -> Pos2 {
        self.origin + vec2(x, y) * self.unit
    }

    fn rect_at(&self, min: (f32, f32), max: (f32, f32)) -> Rect {
        Rect::from_min_max(self.at(min), self.at(max))
    }

    fn line(&self, a: (f32, f32), b: (f32, f32)) {
        self.painter
            .line_segment([self.at(a), self.at(b)], self.stroke);
    }

    fn polyline(&self, points: Vec<Pos2>) {
        self.painter.add(Shape::line(points, self.stroke));
    }

    fn polyline_at(&self, points: &[(f32, f32)]) {
        self.polyline(points.iter().map(|&p| self.at(p)).collect());
    }

    fn rect(&self, min: (f32, f32), max: (f32, f32), radius: f32) {
        self.painter.rect_stroke(
            self.rect_at(min, max),
            radius * self.unit,
            self.stroke,
            eframe::egui::StrokeKind::Middle,
        );
    }

    fn circle(&self, center: (f32, f32), radius: f32) {
        self.painter
            .circle_stroke(self.at(center), radius * self.unit, self.stroke);
    }

    fn dot(&self, center: (f32, f32), radius: f32) {
        self.painter
            .circle_filled(self.at(center), radius * self.unit, self.stroke.color);
    }

    /// Points along a circular arc (angles in radians, 0 = +x, clockwise on screen).
    fn arc(&self, center: (f32, f32), radius: f32, from: f32, to: f32) -> Vec<Pos2> {
        let c = self.at(center);
        let r = radius * self.unit;
        let steps = 24;
        (0..=steps)
            .map(|i| {
                let a = from + (to - from) * i as f32 / steps as f32;
                c + r * Vec2::angled(a)
            })
            .collect()
    }

    /// Points along a quadratic Bézier curve.
    fn curve(&self, a: (f32, f32), control: (f32, f32), b: (f32, f32)) -> Vec<Pos2> {
        let (a, c, b) = (self.at(a), self.at(control), self.at(b));
        (0..=16)
            .map(|i| {
                let t = i as f32 / 16.0;
                let u = 1.0 - t;
                Pos2::new(
                    u * u * a.x + 2.0 * u * t * c.x + t * t * b.x,
                    u * u * a.y + 2.0 * u * t * c.y + t * t * b.y,
                )
            })
            .collect()
    }

    fn arrow(&self, from: (f32, f32), to: (f32, f32)) {
        self.line(from, to);
        self.head(self.at(from), self.at(to));
    }

    /// Arrowhead at `tip`, pointing away from `from`.
    fn head(&self, from: Pos2, tip: Pos2) {
        let dir = (tip - from).normalized();
        let len = 4.0 * self.unit;
        for turn in [PI * 0.78, -PI * 0.78] {
            let side = Vec2::angled(dir.angle() + turn) * len;
            self.painter.line_segment([tip, tip + side], self.stroke);
        }
    }

    fn page_points(&self) -> Vec<Pos2> {
        [
            (3.0, 1.0),
            (10.0, 1.0),
            (13.0, 4.0),
            (13.0, 15.0),
            (3.0, 15.0),
        ]
        .into_iter()
        .map(|p| self.at(p))
        .collect()
    }

    fn page_outline(&self) {
        let mut points = self.page_points();
        points.push(points[0]);
        self.polyline(points);
        self.polyline_at(&[(10.0, 1.0), (10.0, 4.0), (13.0, 4.0)]);
    }

    fn folder(&self) {
        let r = self.unit;
        self.painter
            .rect_filled(self.rect_at((1.0, 2.0), (7.0, 5.0)), r, FOLDER_BACK);
        self.painter
            .rect_filled(self.rect_at((1.0, 3.5), (15.0, 13.5)), r, FOLDER_BACK);
        self.painter.rect_filled(
            self.rect_at((1.0, 5.5), (15.0, 14.0)),
            1.2 * r,
            FOLDER_FRONT,
        );
    }

    fn file(&self, kind: FileKind) {
        let color = kind.color();
        let glyph = Grid {
            stroke: Stroke::new(1.3 * self.unit, color),
            ..*self
        };
        let tinted_page = |fill: Color32| {
            self.painter.add(Shape::convex_polygon(
                self.page_points(),
                fill,
                Stroke::new(1.1 * self.unit, color),
            ));
            glyph.polyline_at(&[(10.0, 1.0), (10.0, 4.0), (13.0, 4.0)]);
        };
        let tint = color.gamma_multiply(0.16);
        match kind {
            FileKind::Generic => tinted_page(tint),
            FileKind::Code => {
                tinted_page(tint);
                glyph.polyline_at(&[(6.8, 7.0), (5.0, 9.3), (6.8, 11.6)]);
                glyph.polyline_at(&[(9.2, 7.0), (11.0, 9.3), (9.2, 11.6)]);
            }
            FileKind::Text => {
                tinted_page(tint);
                glyph.line((5.5, 7.0), (10.5, 7.0));
                glyph.line((5.5, 9.5), (10.5, 9.5));
                glyph.line((5.5, 12.0), (8.5, 12.0));
            }
            FileKind::Pdf => {
                tinted_page(color.gamma_multiply(0.85));
                let white = Grid {
                    stroke: Stroke::new(1.3 * self.unit, Color32::WHITE),
                    ..*self
                };
                white.line((5.5, 9.0), (10.5, 9.0));
                white.line((5.5, 11.5), (10.5, 11.5));
            }
            FileKind::Sheet => {
                tinted_page(tint);
                glyph.rect((5.0, 7.0), (11.0, 12.5), 0.0);
                glyph.line((5.0, 9.75), (11.0, 9.75));
                glyph.line((8.0, 7.0), (8.0, 12.5));
            }
            FileKind::Config => {
                tinted_page(tint);
                glyph.line((5.0, 8.0), (11.0, 8.0));
                glyph.line((5.0, 11.5), (11.0, 11.5));
                glyph.dot((9.0, 8.0), 1.4);
                glyph.dot((6.8, 11.5), 1.4);
            }
            FileKind::Archive => {
                tinted_page(tint);
                for y in [2.0, 4.0, 6.0] {
                    self.painter
                        .rect_filled(self.rect_at((7.2, y), (8.8, y + 1.0)), 0.0, color);
                }
                self.painter
                    .rect_filled(self.rect_at((6.5, 8.0), (9.5, 11.5)), self.unit, color);
            }
            FileKind::Image => {
                self.painter.rect_filled(
                    self.rect_at((1.5, 2.5), (14.5, 13.5)),
                    1.5 * self.unit,
                    tint,
                );
                glyph.rect((1.5, 2.5), (14.5, 13.5), 1.5);
                glyph.polyline_at(&[
                    (2.0, 12.0),
                    (6.0, 8.0),
                    (9.0, 11.0),
                    (11.0, 9.0),
                    (14.0, 12.0),
                ]);
                glyph.dot((11.0, 5.8), 1.3);
            }
            FileKind::Video => {
                self.painter.rect_filled(
                    self.rect_at((1.5, 3.0), (14.5, 13.0)),
                    2.0 * self.unit,
                    tint,
                );
                glyph.rect((1.5, 3.0), (14.5, 13.0), 2.0);
                let play = vec![
                    self.at((6.3, 5.5)),
                    self.at((11.2, 8.0)),
                    self.at((6.3, 10.5)),
                ];
                self.painter
                    .add(Shape::convex_polygon(play, color, Stroke::NONE));
            }
            FileKind::Audio => {
                glyph.line((10.5, 11.5), (10.5, 2.5));
                glyph.polyline(glyph.curve((10.5, 2.5), (14.5, 3.5), (13.5, 7.0)));
                self.painter
                    .circle_filled(self.at((8.0, 11.8)), 2.6 * self.unit, color);
            }
            FileKind::Executable => {
                self.painter.rect_filled(
                    self.rect_at((1.5, 2.5), (14.5, 13.5)),
                    1.5 * self.unit,
                    tint,
                );
                glyph.rect((1.5, 2.5), (14.5, 13.5), 1.5);
                self.painter.rect_filled(
                    self.rect_at((1.5, 2.5), (14.5, 5.5)),
                    1.5 * self.unit,
                    color,
                );
                glyph.polyline_at(&[(5.0, 8.0), (7.0, 9.75), (5.0, 11.5)]);
                glyph.line((8.5, 11.5), (11.0, 11.5));
            }
        }
    }
}

/// A square button showing a line icon. `name` is its accessible label (what
/// screen readers announce and what tests find it by). `active` tints the
/// icon with the accent (for toggles).
pub fn button(
    ui: &mut Ui,
    p: &Palette,
    icon: Icon,
    name: &str,
    size: f32,
    enabled: bool,
    active: bool,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), sense);
    response.widget_info(|| WidgetInfo::selected(Role::Button, enabled, active, name));
    if ui.is_rect_visible(rect) {
        let hovered = enabled && (response.hovered() || response.has_focus());
        if hovered {
            let fill = if response.is_pointer_button_down_on() {
                p.pressed
            } else {
                p.hover
            };
            ui.painter().rect_filled(rect, 4.0, fill);
        }
        let color = match (enabled, active, hovered) {
            (false, _, _) => p.icon.gamma_multiply(0.4),
            (true, true, _) => p.accent,
            (true, false, true) => p.text,
            (true, false, false) => p.icon,
        };
        let icon_size = 16.0_f32.min(size - 8.0);
        let icon_rect = Rect::from_center_size(rect.center(), vec2(icon_size, icon_size));
        paint(ui.painter(), icon_rect, icon, color);
    }
    response
}

/// Paints a 16×16 icon inline at the cursor.
pub fn inline(ui: &mut Ui, icon: Icon, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
    paint(ui.painter(), rect, icon, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_kinds_by_extension() {
        assert_eq!(FileKind::from_name("photo.JPG"), FileKind::Image);
        assert_eq!(FileKind::from_name("main.rs"), FileKind::Code);
        assert_eq!(FileKind::from_name("Cargo.toml"), FileKind::Config);
        assert_eq!(FileKind::from_name("notes.md"), FileKind::Text);
        assert_eq!(FileKind::from_name("setup.exe"), FileKind::Executable);
        assert_eq!(FileKind::from_name("LICENSE"), FileKind::Generic);
        assert_eq!(FileKind::from_name(".gitignore"), FileKind::Generic);
    }

    #[test]
    fn every_kind_has_its_own_color() {
        let kinds = [
            FileKind::Generic,
            FileKind::Code,
            FileKind::Text,
            FileKind::Pdf,
            FileKind::Sheet,
            FileKind::Image,
            FileKind::Video,
            FileKind::Audio,
            FileKind::Archive,
            FileKind::Executable,
            FileKind::Config,
        ];
        for (i, a) in kinds.iter().enumerate() {
            for b in &kinds[i + 1..] {
                assert_ne!(a.color(), b.color(), "{a:?} and {b:?} share a color");
            }
        }
    }
}
