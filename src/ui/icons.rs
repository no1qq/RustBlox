use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Home,
    Rocket,
    Sliders,
    Flag,
    Info,
    Folder,
    Refresh,
    Warning,
    Check,
    Cross,
    Plus,
    Trash,
    ChevronRight,
    ChevronDown,
    External,
    Play,
    Package,
    Search,
    Copy,
    Minimize,
    Maximize,
    Restore,
    Close,
}

struct Grid {
    origin: Pos2,
    scale: f32,
}

impl Grid {
    fn new(rect: Rect) -> Self {
        let side = rect.width().min(rect.height());
        let scale = side / 24.0;
        let origin = rect.center() - Vec2::splat(side / 2.0);
        Self { origin, scale }
    }

    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.origin + Vec2::new(x * self.scale, y * self.scale)
    }

    fn len(&self, value: f32) -> f32 {
        value * self.scale
    }
}

pub fn draw(painter: &Painter, icon: Icon, rect: Rect, color: Color32, weight: f32) {
    let grid = Grid::new(rect);
    let stroke = Stroke::new(weight.max(1.0), color);
    let path = |points: Vec<Pos2>| {
        painter.add(Shape::line(points, stroke));
    };
    let closed = |points: Vec<Pos2>| {
        painter.add(Shape::closed_line(points, stroke));
    };

    match icon {
        Icon::Home => {
            path(vec![
                grid.at(3.5, 10.5),
                grid.at(12.0, 3.5),
                grid.at(20.5, 10.5),
            ]);
            path(vec![
                grid.at(5.5, 9.5),
                grid.at(5.5, 20.0),
                grid.at(18.5, 20.0),
                grid.at(18.5, 9.5),
            ]);
            path(vec![
                grid.at(9.8, 20.0),
                grid.at(9.8, 14.0),
                grid.at(14.2, 14.0),
                grid.at(14.2, 20.0),
            ]);
        }
        Icon::Rocket => {
            closed(vec![
                grid.at(12.0, 2.6),
                grid.at(16.4, 8.6),
                grid.at(16.4, 15.2),
                grid.at(12.0, 18.4),
                grid.at(7.6, 15.2),
                grid.at(7.6, 8.6),
            ]);
            painter.circle_stroke(grid.at(12.0, 9.6), grid.len(2.0), stroke);
            path(vec![
                grid.at(7.6, 13.4),
                grid.at(4.4, 17.4),
                grid.at(8.2, 17.0),
            ]);
            path(vec![
                grid.at(16.4, 13.4),
                grid.at(19.6, 17.4),
                grid.at(15.8, 17.0),
            ]);
            path(vec![
                grid.at(10.4, 19.4),
                grid.at(12.0, 21.6),
                grid.at(13.6, 19.4),
            ]);
        }
        Icon::Sliders => {
            for (index, y) in [6.5_f32, 12.0, 17.5].iter().enumerate() {
                path(vec![grid.at(4.0, *y), grid.at(20.0, *y)]);
                let knob = [15.0, 9.0, 13.0][index];
                painter.circle_filled(grid.at(knob, *y), grid.len(2.4), color);
            }
        }
        Icon::Flag => {
            path(vec![grid.at(6.0, 3.6), grid.at(6.0, 20.8)]);
            closed(vec![
                grid.at(6.0, 4.6),
                grid.at(18.6, 4.6),
                grid.at(15.4, 8.8),
                grid.at(18.6, 13.0),
                grid.at(6.0, 13.0),
            ]);
        }
        Icon::Info => {
            painter.circle_stroke(grid.at(12.0, 12.0), grid.len(8.8), stroke);
            painter.circle_filled(grid.at(12.0, 8.0), grid.len(1.15), color);
            path(vec![grid.at(12.0, 11.2), grid.at(12.0, 16.6)]);
        }
        Icon::Folder => {
            closed(vec![
                grid.at(3.4, 6.4),
                grid.at(9.6, 6.4),
                grid.at(11.4, 8.8),
                grid.at(20.6, 8.8),
                grid.at(20.6, 18.6),
                grid.at(3.4, 18.6),
            ]);
        }
        Icon::Refresh => {
            let center = grid.at(12.0, 12.0);
            let radius = grid.len(7.6);
            painter.add(Shape::line(arc(center, radius, -0.55, 4.2, 26), stroke));
            let tip = arc(center, radius, 4.2, 4.2, 1)[0];
            closed(vec![
                tip,
                tip + Vec2::new(grid.len(-1.0), grid.len(-3.4)),
                tip + Vec2::new(grid.len(3.2), grid.len(-1.9)),
            ]);
        }
        Icon::Warning => {
            closed(vec![
                grid.at(12.0, 3.4),
                grid.at(21.4, 19.8),
                grid.at(2.6, 19.8),
            ]);
            path(vec![grid.at(12.0, 9.6), grid.at(12.0, 14.4)]);
            painter.circle_filled(grid.at(12.0, 17.2), grid.len(1.1), color);
        }
        Icon::Check => {
            path(vec![
                grid.at(4.8, 12.6),
                grid.at(9.8, 17.6),
                grid.at(19.2, 6.8),
            ]);
        }
        Icon::Cross => {
            path(vec![grid.at(6.2, 6.2), grid.at(17.8, 17.8)]);
            path(vec![grid.at(17.8, 6.2), grid.at(6.2, 17.8)]);
        }
        Icon::Plus => {
            path(vec![grid.at(12.0, 5.2), grid.at(12.0, 18.8)]);
            path(vec![grid.at(5.2, 12.0), grid.at(18.8, 12.0)]);
        }
        Icon::Trash => {
            path(vec![grid.at(4.4, 7.0), grid.at(19.6, 7.0)]);
            path(vec![
                grid.at(9.2, 7.0),
                grid.at(9.2, 4.6),
                grid.at(14.8, 4.6),
                grid.at(14.8, 7.0),
            ]);
            path(vec![
                grid.at(6.4, 7.0),
                grid.at(7.4, 20.0),
                grid.at(16.6, 20.0),
                grid.at(17.6, 7.0),
            ]);
            path(vec![grid.at(10.6, 10.6), grid.at(10.9, 16.6)]);
            path(vec![grid.at(13.4, 10.6), grid.at(13.1, 16.6)]);
        }
        Icon::ChevronRight => {
            path(vec![
                grid.at(9.4, 5.6),
                grid.at(16.0, 12.0),
                grid.at(9.4, 18.4),
            ]);
        }
        Icon::ChevronDown => {
            path(vec![
                grid.at(5.6, 9.4),
                grid.at(12.0, 16.0),
                grid.at(18.4, 9.4),
            ]);
        }
        Icon::External => {
            path(vec![
                grid.at(13.0, 4.6),
                grid.at(19.4, 4.6),
                grid.at(19.4, 11.0),
            ]);
            path(vec![grid.at(19.4, 4.6), grid.at(11.2, 12.8)]);
            path(vec![
                grid.at(16.4, 13.6),
                grid.at(16.4, 19.4),
                grid.at(4.6, 19.4),
                grid.at(4.6, 7.6),
                grid.at(10.4, 7.6),
            ]);
        }
        Icon::Play => {
            painter.add(Shape::convex_polygon(
                vec![grid.at(8.0, 5.2), grid.at(19.0, 12.0), grid.at(8.0, 18.8)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Package => {
            closed(vec![
                grid.at(12.0, 3.2),
                grid.at(20.2, 7.6),
                grid.at(20.2, 16.4),
                grid.at(12.0, 20.8),
                grid.at(3.8, 16.4),
                grid.at(3.8, 7.6),
            ]);
            path(vec![
                grid.at(3.8, 7.6),
                grid.at(12.0, 12.0),
                grid.at(20.2, 7.6),
            ]);
            path(vec![grid.at(12.0, 12.0), grid.at(12.0, 20.8)]);
        }
        Icon::Search => {
            painter.circle_stroke(grid.at(10.6, 10.6), grid.len(6.4), stroke);
            path(vec![grid.at(15.4, 15.4), grid.at(20.2, 20.2)]);
        }
        Icon::Copy => {
            closed(vec![
                grid.at(8.6, 3.8),
                grid.at(20.2, 3.8),
                grid.at(20.2, 15.4),
                grid.at(8.6, 15.4),
            ]);
            path(vec![
                grid.at(15.4, 15.4),
                grid.at(15.4, 20.2),
                grid.at(3.8, 20.2),
                grid.at(3.8, 8.6),
                grid.at(8.6, 8.6),
            ]);
        }
        Icon::Minimize => {
            path(vec![grid.at(6.0, 12.0), grid.at(18.0, 12.0)]);
        }
        Icon::Maximize => {
            closed(vec![
                grid.at(6.4, 6.4),
                grid.at(17.6, 6.4),
                grid.at(17.6, 17.6),
                grid.at(6.4, 17.6),
            ]);
        }
        Icon::Restore => {
            closed(vec![
                grid.at(5.6, 8.8),
                grid.at(15.2, 8.8),
                grid.at(15.2, 18.4),
                grid.at(5.6, 18.4),
            ]);
            path(vec![
                grid.at(8.8, 8.8),
                grid.at(8.8, 5.6),
                grid.at(18.4, 5.6),
                grid.at(18.4, 15.2),
                grid.at(15.2, 15.2),
            ]);
        }
        Icon::Close => {
            path(vec![grid.at(7.0, 7.0), grid.at(17.0, 17.0)]);
            path(vec![grid.at(17.0, 7.0), grid.at(7.0, 17.0)]);
        }
    }
}

fn arc(center: Pos2, radius: f32, start: f32, end: f32, segments: usize) -> Vec<Pos2> {
    let segments = segments.max(1);
    (0..=segments)
        .map(|index| {
            let t = index as f32 / segments as f32;
            let angle = start + (end - start) * t;
            center + Vec2::new(angle.cos() * radius, angle.sin() * radius)
        })
        .collect()
}

pub fn spinner(painter: &Painter, rect: Rect, color: Color32, weight: f32, time: f64) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0 - weight;
    let stroke = Stroke::new(weight, color);

    let head = (time * 2.6) as f32;
    let sweep = 1.4 + (time as f32 * 1.7).sin().abs() * 2.4;

    painter.add(Shape::line(
        arc(center, radius, head, head + sweep, 30),
        stroke,
    ));
}

pub fn ring(painter: &Painter, rect: Rect, color: Color32, weight: f32) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0 - weight;
    painter.circle_stroke(center, radius, Stroke::new(weight, color));
}
