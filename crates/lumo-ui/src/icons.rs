//! Vector icons. Drawn rather than taken from fonts so they look the same on
//! every machine and can never render as a missing glyph.

use std::f32::consts::TAU;

use eframe::egui::{
    Color32, Painter, Pos2, Rect, Response, Role, Sense, Shape, Stroke, Ui, Visuals, WidgetInfo,
    vec2,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Folder,
    File,
    Link,
    Drive,
    Back,
    Forward,
    Up,
    Refresh,
    Close,
    SortUp,
    SortDown,
}

/// Paints `icon` into `rect`. Icons are designed on a 16×16 grid and scale
/// to the rect; line icons use `color`, file-type icons use fixed colors.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32, visuals: &Visuals) {
    let unit = rect.width() / 16.0;
    let at = |x: f32, y: f32| rect.min + vec2(x, y) * unit;
    let line = Stroke::new(1.6 * unit, color);
    let edge = Stroke::new(
        1.0,
        Color32::from_gray(if visuals.dark_mode { 150 } else { 120 }),
    );

    match icon {
        Icon::Folder => {
            let back = Color32::from_rgb(0xD8, 0x9B, 0x22);
            let front = Color32::from_rgb(0xF2, 0xC2, 0x4E);
            painter.rect_filled(Rect::from_min_max(at(1.0, 2.5), at(7.5, 6.0)), unit, back);
            painter.rect_filled(
                Rect::from_min_max(at(1.0, 4.5), at(15.0, 14.0)),
                1.5 * unit,
                front,
            );
        }
        Icon::File | Icon::Link => {
            let paper = if visuals.dark_mode {
                Color32::from_gray(225)
            } else {
                Color32::WHITE
            };
            let page = vec![
                at(3.0, 1.0),
                at(10.0, 1.0),
                at(13.0, 4.0),
                at(13.0, 15.0),
                at(3.0, 15.0),
            ];
            painter.add(Shape::convex_polygon(page, paper, edge));
            painter.line_segment([at(10.0, 1.0), at(10.0, 4.0)], edge);
            painter.line_segment([at(10.0, 4.0), at(13.0, 4.0)], edge);
            if icon == Icon::Link {
                let badge = Rect::from_min_max(at(1.0, 9.0), at(8.0, 16.0));
                painter.rect_filled(badge, unit, Color32::from_rgb(0x2B, 0x7C, 0xD3));
                let arrow = Stroke::new(1.4 * unit, Color32::WHITE);
                painter.line_segment([at(3.0, 14.0), at(6.0, 11.0)], arrow);
                painter.line_segment([at(4.0, 11.0), at(6.0, 11.0)], arrow);
                painter.line_segment([at(6.0, 11.0), at(6.0, 13.0)], arrow);
            }
        }
        Icon::Drive => {
            painter.rect_filled(
                Rect::from_min_max(at(1.0, 5.0), at(15.0, 12.0)),
                2.0 * unit,
                Color32::from_gray(150),
            );
            painter.rect_filled(
                Rect::from_min_max(at(2.0, 10.0), at(14.0, 11.0)),
                0.0,
                Color32::from_gray(95),
            );
            painter.circle_filled(at(12.5, 7.5), unit, Color32::from_rgb(0x4C, 0xC8, 0x78));
        }
        Icon::Back => arrow(painter, at(13.0, 8.0), at(3.0, 8.0), unit, line),
        Icon::Forward => arrow(painter, at(3.0, 8.0), at(13.0, 8.0), unit, line),
        Icon::Up => arrow(painter, at(8.0, 13.0), at(8.0, 3.0), unit, line),
        Icon::Refresh => {
            // Three-quarter circle with an arrowhead at its open end.
            let center = at(8.0, 8.5);
            let radius = 5.0 * unit;
            let points: Vec<Pos2> = (0..=20)
                .map(|i| {
                    let angle = -0.15 * TAU + 0.8 * TAU * i as f32 / 20.0;
                    center + radius * vec2(angle.cos(), angle.sin())
                })
                .collect();
            let tip = points[0];
            painter.add(Shape::line(points, line));
            painter.line_segment([tip, tip + vec2(-3.5, 0.5) * unit], line);
            painter.line_segment([tip, tip + vec2(0.5, 3.5) * unit], line);
        }
        Icon::Close => {
            painter.line_segment([at(4.0, 4.0), at(12.0, 12.0)], line);
            painter.line_segment([at(12.0, 4.0), at(4.0, 12.0)], line);
        }
        Icon::SortUp => {
            let points = vec![at(8.0, 5.0), at(12.0, 11.0), at(4.0, 11.0)];
            painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
        }
        Icon::SortDown => {
            let points = vec![at(4.0, 5.0), at(12.0, 5.0), at(8.0, 11.0)];
            painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
        }
    }
}

fn arrow(painter: &Painter, from: Pos2, to: Pos2, unit: f32, stroke: Stroke) {
    let dir = (to - from).normalized();
    let side = vec2(-dir.y, dir.x);
    painter.line_segment([from, to], stroke);
    painter.line_segment([to, to - dir * 4.5 * unit + side * 4.5 * unit], stroke);
    painter.line_segment([to, to - dir * 4.5 * unit - side * 4.5 * unit], stroke);
}

/// A square button showing `icon`. `name` is its accessible label (what
/// screen readers announce and what tests find it by).
pub fn button(ui: &mut Ui, icon: Icon, name: &str, size: f32, enabled: bool) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), sense);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, name));
    if ui.is_rect_visible(rect) {
        let visuals = if enabled {
            *ui.style().interact(&response)
        } else {
            ui.visuals().widgets.noninteractive
        };
        if enabled && (response.hovered() || response.has_focus()) {
            ui.painter()
                .rect_filled(rect, visuals.corner_radius, visuals.weak_bg_fill);
        }
        let color = if enabled {
            visuals.fg_stroke.color
        } else {
            ui.visuals().weak_text_color()
        };
        let icon_size = (size * 0.6).round();
        let icon_rect = Rect::from_center_size(rect.center(), vec2(icon_size, icon_size));
        paint(ui.painter(), icon_rect, icon, color, ui.visuals());
    }
    response
}

/// Paints a 16×16 icon inline at the cursor.
pub fn inline(ui: &mut Ui, icon: Icon) {
    let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
    let color = ui.visuals().text_color();
    paint(ui.painter(), rect, icon, color, ui.visuals());
}
