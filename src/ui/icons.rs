use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Home,
    Rocket,
    Sliders,
    Gauge,
    Layers,
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
    User,
    Users,
    Gamepad,
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
                grid.at(3.0, 10.0),
                grid.at(12.0, 3.0),
                grid.at(21.0, 10.0),
                grid.at(21.0, 19.0),
                grid.at(19.0, 21.0),
                grid.at(5.0, 21.0),
                grid.at(3.0, 19.0),
                grid.at(3.0, 10.0),
            ]);
            path(vec![
                grid.at(9.0, 21.0),
                grid.at(9.0, 12.0),
                grid.at(15.0, 12.0),
                grid.at(15.0, 21.0),
            ]);
        }
        Icon::Rocket => {
            closed(vec![
                grid.at(9.0, 12.0),
                grid.at(11.0, 8.0),
                grid.at(22.0, 2.0),
                grid.at(16.0, 13.0),
                grid.at(12.0, 15.0),
            ]);
            path(vec![
                grid.at(12.0, 15.0),
                grid.at(12.0, 20.0),
                grid.at(16.0, 18.0),
            ]);
            path(vec![
                grid.at(4.5, 16.5),
                grid.at(2.5, 21.5),
                grid.at(7.5, 19.5),
            ]);
            path(vec![
                grid.at(9.0, 12.0),
                grid.at(4.0, 12.0),
                grid.at(6.0, 8.0),
                grid.at(9.0, 12.0),
            ]);
        }
        Icon::Layers => {
            closed(vec![
                grid.at(12.0, 2.0),
                grid.at(21.0, 7.0),
                grid.at(21.0, 17.0),
                grid.at(12.0, 22.0),
                grid.at(3.0, 17.0),
                grid.at(3.0, 7.0),
            ]);
            path(vec![grid.at(12.0, 22.0), grid.at(12.0, 12.0)]);
            path(vec![
                grid.at(3.3, 7.0),
                grid.at(12.0, 12.0),
                grid.at(20.7, 7.0),
            ]);
            path(vec![grid.at(7.5, 4.3), grid.at(16.5, 9.5)]);
        }
        Icon::Gauge => {
            path(vec![grid.at(2.0, 3.0), grid.at(2.0, 21.0)]);
            closed(vec![
                grid.at(2.0, 5.0),
                grid.at(20.0, 5.0),
                grid.at(22.0, 7.0),
                grid.at(22.0, 15.0),
                grid.at(20.0, 17.0),
                grid.at(2.0, 17.0),
            ]);
            path(vec![
                grid.at(7.0, 17.0),
                grid.at(7.0, 20.0),
                grid.at(14.0, 20.0),
                grid.at(14.0, 17.0),
            ]);
            painter.circle_stroke(grid.at(8.0, 11.0), grid.len(2.0), stroke);
            painter.circle_stroke(grid.at(16.0, 11.0), grid.len(2.0), stroke);
        }
        Icon::Sliders => {
            path(vec![grid.at(3.0, 5.0), grid.at(10.0, 5.0)]);
            path(vec![grid.at(14.0, 5.0), grid.at(21.0, 5.0)]);
            path(vec![grid.at(14.0, 3.0), grid.at(14.0, 7.0)]);

            path(vec![grid.at(3.0, 12.0), grid.at(8.0, 12.0)]);
            path(vec![grid.at(12.0, 12.0), grid.at(21.0, 12.0)]);
            path(vec![grid.at(8.0, 10.0), grid.at(8.0, 14.0)]);

            path(vec![grid.at(3.0, 19.0), grid.at(12.0, 19.0)]);
            path(vec![grid.at(16.0, 19.0), grid.at(21.0, 19.0)]);
            path(vec![grid.at(16.0, 17.0), grid.at(16.0, 21.0)]);
        }
        Icon::Flag => {
            path(vec![grid.at(4.0, 22.0), grid.at(4.0, 4.0)]);
            closed(vec![
                grid.at(4.0, 4.0),
                grid.at(8.0, 2.0),
                grid.at(15.0, 4.0),
                grid.at(20.0, 3.2),
                grid.at(20.0, 14.0),
                grid.at(16.0, 16.0),
                grid.at(8.0, 14.0),
                grid.at(4.0, 15.5),
            ]);
        }
        Icon::Info => {
            painter.circle_stroke(grid.at(12.0, 12.0), grid.len(10.0), stroke);
            path(vec![grid.at(12.0, 16.0), grid.at(12.0, 12.0)]);
            painter.circle_filled(grid.at(12.0, 8.0), grid.len(1.15), color);
        }
        Icon::Folder => {
            closed(vec![
                grid.at(4.0, 20.0),
                grid.at(20.0, 20.0),
                grid.at(22.0, 18.0),
                grid.at(22.0, 8.0),
                grid.at(20.0, 6.0),
                grid.at(12.0, 6.0),
                grid.at(9.6, 3.9),
                grid.at(7.9, 3.0),
                grid.at(4.0, 3.0),
                grid.at(2.0, 5.0),
                grid.at(2.0, 18.0),
            ]);
        }
        Icon::Refresh => {
            let center = grid.at(12.0, 12.0);
            let radius = grid.len(9.0);
            painter.add(Shape::line(
                arc(center, radius, -std::f32::consts::PI, -0.46, 20),
                stroke,
            ));
            path(vec![
                grid.at(21.0, 3.0),
                grid.at(21.0, 8.0),
                grid.at(16.0, 8.0),
            ]);
            painter.add(Shape::line(
                arc(center, radius, 0.0, std::f32::consts::PI - 0.46, 20),
                stroke,
            ));
            path(vec![
                grid.at(8.0, 16.0),
                grid.at(3.0, 16.0),
                grid.at(3.0, 21.0),
            ]);
        }
        Icon::Warning => {
            closed(vec![
                grid.at(12.0, 3.0),
                grid.at(21.5, 20.0),
                grid.at(2.5, 20.0),
            ]);
            path(vec![grid.at(12.0, 9.0), grid.at(12.0, 14.0)]);
            painter.circle_filled(grid.at(12.0, 17.0), grid.len(1.0), color);
        }
        Icon::Check => {
            path(vec![
                grid.at(4.0, 12.0),
                grid.at(9.0, 17.0),
                grid.at(20.0, 6.0),
            ]);
        }
        Icon::Cross => {
            path(vec![grid.at(6.0, 6.0), grid.at(18.0, 18.0)]);
            path(vec![grid.at(18.0, 6.0), grid.at(6.0, 18.0)]);
        }
        Icon::Plus => {
            path(vec![grid.at(12.0, 5.0), grid.at(12.0, 19.0)]);
            path(vec![grid.at(5.0, 12.0), grid.at(19.0, 12.0)]);
        }
        Icon::Trash => {
            path(vec![grid.at(3.0, 6.0), grid.at(21.0, 6.0)]);
            path(vec![
                grid.at(8.0, 6.0),
                grid.at(8.0, 4.0),
                grid.at(10.0, 2.0),
                grid.at(14.0, 2.0),
                grid.at(16.0, 4.0),
                grid.at(16.0, 6.0),
            ]);
            closed(vec![
                grid.at(5.0, 6.0),
                grid.at(7.0, 20.0),
                grid.at(17.0, 20.0),
                grid.at(19.0, 6.0),
            ]);
            path(vec![grid.at(10.0, 11.0), grid.at(10.0, 17.0)]);
            path(vec![grid.at(14.0, 11.0), grid.at(14.0, 17.0)]);
        }
        Icon::ChevronRight => {
            path(vec![grid.at(5.0, 12.0), grid.at(19.0, 12.0)]);
            path(vec![
                grid.at(12.0, 5.0),
                grid.at(19.0, 12.0),
                grid.at(12.0, 19.0),
            ]);
        }
        Icon::ChevronDown => {
            path(vec![grid.at(12.0, 5.0), grid.at(12.0, 19.0)]);
            path(vec![
                grid.at(5.0, 12.0),
                grid.at(12.0, 19.0),
                grid.at(19.0, 12.0),
            ]);
        }
        Icon::External => {
            closed(vec![
                grid.at(3.0, 3.0),
                grid.at(21.0, 3.0),
                grid.at(21.0, 21.0),
                grid.at(3.0, 21.0),
            ]);
            path(vec![grid.at(9.0, 15.0), grid.at(15.0, 9.0)]);
        }
        Icon::Play => {
            painter.add(Shape::convex_polygon(
                vec![grid.at(6.0, 4.5), grid.at(19.5, 12.0), grid.at(6.0, 19.5)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Package => {
            closed(vec![
                grid.at(21.0, 8.0),
                grid.at(12.0, 3.0),
                grid.at(3.0, 8.0),
                grid.at(3.0, 16.0),
                grid.at(12.0, 21.0),
                grid.at(21.0, 16.0),
            ]);
            path(vec![
                grid.at(3.3, 7.0),
                grid.at(12.0, 12.0),
                grid.at(20.7, 7.0),
            ]);
            path(vec![grid.at(12.0, 22.0), grid.at(12.0, 12.0)]);
        }
        Icon::Search => {
            painter.circle_stroke(grid.at(11.0, 11.0), grid.len(7.0), stroke);
            path(vec![grid.at(16.0, 16.0), grid.at(21.0, 21.0)]);
        }
        Icon::Copy => {
            path(vec![
                grid.at(3.0, 16.0),
                grid.at(7.0, 20.0),
                grid.at(11.0, 16.0),
            ]);
            path(vec![grid.at(7.0, 4.0), grid.at(7.0, 20.0)]);
            path(vec![
                grid.at(21.0, 8.0),
                grid.at(17.0, 4.0),
                grid.at(13.0, 8.0),
            ]);
            path(vec![grid.at(17.0, 4.0), grid.at(17.0, 20.0)]);
        }
        Icon::User => {
            painter.circle_stroke(grid.at(12.0, 8.0), grid.len(5.0), stroke);
            let mut pts = Vec::with_capacity(13);
            for i in 0..=12 {
                let angle = std::f32::consts::PI + (std::f32::consts::PI * i as f32 / 12.0);
                pts.push(grid.at(12.0 + 8.0 * angle.cos(), 21.0 + 8.0 * angle.sin()));
            }
            path(pts);
        }
        Icon::Users => {
            painter.circle_stroke(grid.at(10.0, 8.0), grid.len(4.5), stroke);
            let mut pts = Vec::with_capacity(13);
            for i in 0..=12 {
                let angle = std::f32::consts::PI + (std::f32::consts::PI * i as f32 / 12.0);
                pts.push(grid.at(10.0 + 7.5 * angle.cos(), 21.0 + 7.5 * angle.sin()));
            }
            path(pts);

            let mut head_pts = Vec::with_capacity(9);
            for i in 0..=8 {
                let angle =
                    -std::f32::consts::FRAC_PI_2 + (std::f32::consts::PI * 0.75 * i as f32 / 8.0);
                head_pts.push(grid.at(17.5 + 4.2 * angle.cos(), 7.5 + 4.2 * angle.sin()));
            }
            path(head_pts);

            path(vec![
                grid.at(18.0, 13.0),
                grid.at(20.5, 15.5),
                grid.at(22.0, 20.0),
            ]);
        }
        Icon::Gamepad => {
            path(vec![grid.at(6.0, 11.0), grid.at(10.0, 11.0)]);
            path(vec![grid.at(8.0, 9.0), grid.at(8.0, 13.0)]);
            painter.circle_filled(grid.at(15.0, 12.0), grid.len(1.0), color);
            painter.circle_filled(grid.at(18.0, 10.0), grid.len(1.0), color);
            closed(vec![
                grid.at(6.7, 5.0),
                grid.at(17.3, 5.0),
                grid.at(21.3, 8.6),
                grid.at(22.0, 16.0),
                grid.at(19.0, 19.0),
                grid.at(17.0, 18.0),
                grid.at(15.5, 16.6),
                grid.at(14.2, 16.0),
                grid.at(9.8, 16.0),
                grid.at(8.5, 16.6),
                grid.at(7.0, 18.0),
                grid.at(5.0, 19.0),
                grid.at(2.0, 16.0),
                grid.at(2.7, 8.6),
            ]);
        }
        Icon::Minimize => {
            path(vec![grid.at(5.0, 12.0), grid.at(19.0, 12.0)]);
        }
        Icon::Maximize => {
            closed(vec![
                grid.at(5.0, 5.0),
                grid.at(19.0, 5.0),
                grid.at(19.0, 19.0),
                grid.at(5.0, 19.0),
            ]);
        }
        Icon::Restore => {
            closed(vec![
                grid.at(5.0, 9.0),
                grid.at(15.0, 9.0),
                grid.at(15.0, 19.0),
                grid.at(5.0, 19.0),
            ]);
            path(vec![
                grid.at(9.0, 9.0),
                grid.at(9.0, 5.0),
                grid.at(19.0, 5.0),
                grid.at(19.0, 15.0),
                grid.at(15.0, 15.0),
            ]);
        }
        Icon::Close => {
            path(vec![grid.at(6.0, 6.0), grid.at(18.0, 18.0)]);
            path(vec![grid.at(18.0, 6.0), grid.at(6.0, 18.0)]);
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
