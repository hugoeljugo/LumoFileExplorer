//! Lumo's look: one place for colors, sizes and fonts ("design tokens").
//!
//! Inspired by Zed: flat surfaces separated by hairlines, dense rows, a muted
//! palette where the accent is the only color. Lumo's signature is the
//! *lumen*: the active tab and the selected item carry an accent line with a
//! soft glow (see [`lumen`]). The Phase 10 theme engine will let users
//! override these values.

use eframe::egui::{
    self, Color32, Context, CornerRadius, FontData, FontFamily, FontId, Mesh, Pos2, Rect, Shape,
    Stroke, TextStyle, Theme, ThemePreference, Visuals,
    epaint::text::{FontInsert, FontPriority, InsertFontFamily},
    style::{Selection, WidgetVisuals, Widgets},
    vec2,
};

/// Lumo orange: the default accent.
pub const LUMO_ORANGE: Color32 = Color32::from_rgb(0xFF, 0x7A, 0x1A);

/// Row height of file-table rows.
pub const ROW_HEIGHT: f32 = 28.0;
/// Height of sidebar entries.
pub const NAV_ITEM_HEIGHT: f32 = 28.0;
/// Square toolbar buttons and the address bar.
pub const TOOLBAR_CONTROL: f32 = 28.0;
pub const TAB_HEIGHT: f32 = 36.0;
pub const CONTROL_RADIUS: u8 = 4;
/// Thickness of the lumen accent line.
pub const LUMEN_LINE: f32 = 2.0;

// Bundled fonts (SIL Open Font License 1.1; licenses next to the files).
// Compiled in so Lumo looks the same on every machine without an installer.
const PLEX_SANS_REGULAR: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf");
const PLEX_SANS_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf");
const LILEX_REGULAR: &[u8] = include_bytes!("../assets/fonts/Lilex-Regular.ttf");

/// Semibold UI text (section headers, active tab).
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

/// Monospace text for numbers that should line up (sizes, dates).
pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

/// Resolved colors for the current light/dark theme.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Chrome around the content: tab bar, sidebar, status bar.
    pub base: Color32,
    /// The content surface: file list, toolbar and the active tab, which join up.
    pub surface: Color32,
    /// Popups and the address field while typing.
    pub elevated: Color32,
    /// Hairlines between areas.
    pub border: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    /// Selected rows and sidebar entries (accent-tinted).
    pub selected: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    /// Icons and other quiet decoration.
    pub icon: Color32,
    pub accent: Color32,
}

impl Palette {
    pub fn new(theme: Theme, accent: Color32) -> Self {
        match theme {
            Theme::Dark => Self {
                base: rgb(0x18, 0x19, 0x1C),
                surface: rgb(0x1F, 0x21, 0x25),
                elevated: rgb(0x2A, 0x2D, 0x32),
                border: rgb(0x2E, 0x31, 0x37),
                hover: white(0.05),
                pressed: white(0.03),
                selected: accent.gamma_multiply(0.16),
                text: rgb(0xDE, 0xE0, 0xE4),
                text_secondary: rgb(0x93, 0x97, 0xA0),
                icon: rgb(0xA4, 0xA8, 0xB0),
                accent,
            },
            Theme::Light => {
                // A touch deeper so the accent holds up on white.
                let accent = mix(accent, Color32::BLACK, 0.08);
                Self {
                    base: rgb(0xF0, 0xF0, 0xF2),
                    surface: rgb(0xFB, 0xFB, 0xFC),
                    elevated: rgb(0xFF, 0xFF, 0xFF),
                    border: rgb(0xDE, 0xDF, 0xE3),
                    hover: black(0.045),
                    pressed: black(0.07),
                    selected: accent.gamma_multiply(0.13),
                    text: rgb(0x22, 0x24, 0x28),
                    text_secondary: rgb(0x5E, 0x62, 0x6B),
                    icon: rgb(0x5A, 0x5E, 0x66),
                    accent,
                }
            }
        }
    }

    pub fn of(ctx: &Context) -> Self {
        Self::new(ctx.theme(), LUMO_ORANGE)
    }
}

/// Installs fonts, sizes and both light and dark visuals.
pub fn install(ctx: &Context) {
    install_fonts(ctx);
    // `LUMO_THEME=light|dark` forces a theme (screenshots, preference testing).
    match std::env::var("LUMO_THEME").as_deref() {
        Ok("light") => ctx.set_theme(ThemePreference::Light),
        Ok("dark") => ctx.set_theme(ThemePreference::Dark),
        _ => ctx.set_theme(ThemePreference::System),
    }
    for theme in [Theme::Dark, Theme::Light] {
        ctx.set_visuals_of(theme, visuals(&Palette::new(theme, LUMO_ORANGE), theme));
    }
    ctx.all_styles_mut(|style| {
        // Clicks on file names should select rows, not start text selection.
        style.interaction.selectable_labels = false;
        style.spacing.item_spacing = vec2(8.0, 2.0);
        style.spacing.button_padding = vec2(8.0, 4.0);
        style.spacing.interact_size.y = 24.0;
        style.text_styles = [
            (TextStyle::Heading, semibold(18.0)),
            (TextStyle::Body, FontId::proportional(13.5)),
            (TextStyle::Button, FontId::proportional(13.5)),
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Monospace, mono(12.5)),
        ]
        .into();
    });
}

fn install_fonts(ctx: &Context) {
    // egui's bundled fonts stay as fallbacks (symbols); eframe's
    // `system_fonts` feature covers scripts Plex lacks (CJK etc.).
    let mut defs = egui::FontDefinitions::default();
    let proportional = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    defs.families
        .insert(FontFamily::Name("semibold".into()), proportional);
    ctx.set_fonts(defs);

    let fonts = [
        ("IBM Plex Sans", PLEX_SANS_REGULAR, FontFamily::Proportional),
        (
            "IBM Plex Sans SemiBold",
            PLEX_SANS_SEMIBOLD,
            FontFamily::Name("semibold".into()),
        ),
        ("Lilex", LILEX_REGULAR, FontFamily::Monospace),
    ];
    for (name, bytes, family) in fonts {
        ctx.add_font(FontInsert {
            name: name.to_owned(),
            data: FontData::from_static(bytes),
            families: vec![InsertFontFamily {
                family,
                priority: FontPriority::Highest,
            }],
        });
    }
}

fn visuals(p: &Palette, theme: Theme) -> Visuals {
    let mut v = match theme {
        Theme::Dark => Visuals::dark(),
        Theme::Light => Visuals::light(),
    };
    let radius = CornerRadius::same(CONTROL_RADIUS);
    let widget = |weak: Color32, fg: Color32| WidgetVisuals {
        bg_fill: p.elevated,
        weak_bg_fill: weak,
        bg_stroke: Stroke::NONE,
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0, fg),
        expansion: 0.0,
    };
    v.widgets = Widgets {
        // Hairlines (panel separators, table column lines) use this stroke.
        noninteractive: WidgetVisuals {
            bg_stroke: Stroke::new(1.0, p.border),
            ..widget(p.base, p.text_secondary)
        },
        // Buttons are "ghost" style: no fill until hovered.
        inactive: widget(Color32::TRANSPARENT, p.text),
        hovered: widget(p.hover, p.text),
        active: widget(p.pressed, p.text),
        open: widget(p.hover, p.text),
    };
    v.selection = Selection {
        bg_fill: p.accent.gamma_multiply(0.35),
        stroke: Stroke::new(1.0, p.accent),
    };
    v.panel_fill = p.base;
    v.window_fill = p.elevated;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(6);
    v.menu_corner_radius = CornerRadius::same(6);
    v.extreme_bg_color = p.elevated;
    v.faint_bg_color = p.hover;
    v.hyperlink_color = p.accent;
    v.striped = false;
    v
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Left,
    Top,
}

/// The lumen: an accent line along one edge of `rect` with a glow fading
/// away from it. `Edge::Left` for list items, `Edge::Top` for the active tab.
pub fn lumen(p: &Palette, rect: Rect, edge: Edge, glow_depth: f32) -> Shape {
    let (line, glow_rect) = match edge {
        Edge::Left => (
            Rect::from_min_size(rect.min, vec2(LUMEN_LINE, rect.height())),
            Rect::from_min_size(rect.min, vec2(glow_depth.min(rect.width()), rect.height())),
        ),
        Edge::Top => (
            Rect::from_min_size(rect.min, vec2(rect.width(), LUMEN_LINE)),
            Rect::from_min_size(rect.min, vec2(rect.width(), glow_depth.min(rect.height()))),
        ),
    };
    Shape::Vec(vec![
        glow(glow_rect, p.accent.gamma_multiply(0.32), edge),
        Shape::rect_filled(line, 0.0, p.accent),
    ])
}

/// A rectangle filled with `color` at `edge`, fading to transparent at the
/// opposite side.
fn glow(rect: Rect, color: Color32, edge: Edge) -> Shape {
    let clear = Color32::TRANSPARENT;
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    let colors = match edge {
        Edge::Left => [color, clear, clear, color],
        Edge::Top => [color, color, clear, clear],
    };
    let mut mesh = Mesh::default();
    for (pos, color) in corners.into_iter().zip(colors) {
        mesh.colored_vertex(pos, color);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    Shape::mesh(mesh)
}

/// A 1px horizontal hairline from `from` to `to_x`.
pub fn hairline(p: &Palette, from: Pos2, to_x: f32) -> Shape {
    Shape::line_segment([from, Pos2::new(to_x, from.y)], Stroke::new(1.0, p.border))
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

fn white(alpha: f32) -> Color32 {
    Color32::from_white_alpha((alpha * 255.0).round() as u8)
}

fn black(alpha: f32) -> Color32 {
    Color32::from_black_alpha((alpha * 255.0).round() as u8)
}

/// Linear mix of `a` toward `b` by `t` (0 = a, 1 = b), ignoring alpha.
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let lerp = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(lerp(a.r(), b.r()), lerp(a.g(), b.g()), lerp(a.b(), b.b()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_endpoints_and_midpoint() {
        let a = Color32::from_rgb(0, 100, 200);
        assert_eq!(mix(a, Color32::WHITE, 0.0), a);
        assert_eq!(mix(a, Color32::WHITE, 1.0), Color32::WHITE);
        assert_eq!(mix(Color32::BLACK, Color32::WHITE, 0.5), rgb(128, 128, 128));
    }

    #[test]
    fn text_contrasts_with_backgrounds_in_both_themes() {
        // WCAG AA asks for 4.5:1 on body text.
        for theme in [Theme::Dark, Theme::Light] {
            let p = Palette::new(theme, LUMO_ORANGE);
            for bg in [p.base, p.surface, p.elevated] {
                for (name, fg) in [("text", p.text), ("secondary", p.text_secondary)] {
                    let ratio = contrast(fg, bg);
                    assert!(ratio >= 4.5, "{theme:?} {name} on {bg:?}: {ratio:.2}");
                }
            }
        }
    }

    #[test]
    fn bundled_fonts_are_truetype() {
        for font in [PLEX_SANS_REGULAR, PLEX_SANS_SEMIBOLD, LILEX_REGULAR] {
            assert_eq!(&font[..4], &[0, 1, 0, 0], "not a TrueType file");
        }
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let lum = |c: Color32| {
            let ch = |v: u8| {
                let v = f32::from(v) / 255.0;
                if v <= 0.040_45 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
        };
        let (la, lb) = (lum(a), lum(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }
}
