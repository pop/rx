use crate::brush::BrushMode;
use crate::color;
use crate::cursor2d;
use crate::execution::Execution;
use crate::font::TextBatch;
use crate::platform;
use crate::session;
use crate::session::{Mode, Rgb8, Session, Tool, VisualState};
use crate::view::{View, ViewCoords};

use rgx::core::Rgba;
use rgx::kit::shape2d::{Fill, Line, Rotation, Shape, Stroke};
use rgx::kit::{self, Geometry};
use rgx::kit::{shape2d, sprite2d};
use rgx::kit::{Origin, Rgba8, ZDepth};
use rgx::math::{Matrix4, Vector2};
use rgx::rect::Rect;

use std::cell::RefCell;
use std::rc::Rc;
use std::time;

pub const CHECKER_LAYER: ZDepth = ZDepth(-0.9);
pub const VIEW_LAYER: ZDepth = ZDepth(-0.7);
pub const BRUSH_LAYER: ZDepth = ZDepth(-0.6);
pub const GRID_LAYER: ZDepth = ZDepth(-0.5);
pub const UI_LAYER: ZDepth = ZDepth(-0.4);
pub const TEXT_LAYER: ZDepth = ZDepth(-0.3);
pub const PALETTE_LAYER: ZDepth = ZDepth(-0.2);
pub const HELP_LAYER: ZDepth = ZDepth(-0.1);
pub const CURSOR_LAYER: ZDepth = ZDepth(0.0);
pub const XRAY_RADIUS: f32 = 3.0;
pub const XRAY_MIN_ZOOM: f32 = 16.0;

pub const GLYPH_WIDTH: f32 = 8.;
pub const GLYPH_HEIGHT: f32 = 14.;

#[rustfmt::skip]
pub const CHECKER: [u8; 16] = [
    0x55, 0x55, 0x55, 0xff,
    0x66, 0x66, 0x66, 0xff,
    0x66, 0x66, 0x66, 0xff,
    0x55, 0x55, 0x55, 0xff,
];
const LINE_HEIGHT: f32 = GLYPH_HEIGHT + 4.;
const MARGIN: f32 = 10.;

pub mod cursors {
    use super::*;

    pub struct Cursor {
        pub(super) rect: Rect<f32>,
        pub(super) offset: Vector2<f32>,
        pub(super) invert: bool,
    }

    impl Cursor {
        const fn new(rect: Rect<f32>, off_x: f32, off_y: f32, invert: bool) -> Self {
            Self {
                rect,
                offset: Vector2::new(off_x, off_y),
                invert,
            }
        }
    }

    const CROSSHAIR: Cursor = Cursor::new(Rect::new(16., 0., 32., 16.), -8., -8., true);
    const SAMPLER: Cursor = Cursor::new(Rect::new(0., 0., 16., 16.), 1., 1., false);
    const PAN: Cursor = Cursor::new(Rect::new(48., 0., 64., 16.), -8., -8., false);
    const OMNI: Cursor = Cursor::new(Rect::new(32., 0., 48., 16.), -8., -8., false);
    const ERASE: Cursor = Cursor::new(Rect::new(64., 0., 80., 16.), -8., -8., true);

    pub fn info(t: &Tool, m: Mode, in_view: bool, in_selection: bool) -> Option<Cursor> {
        match m {
            Mode::Help | Mode::Present => return None,
            _ => {}
        }
        let cursor = match t {
            Tool::Sampler => self::SAMPLER,
            Tool::Pan(_) => self::PAN,

            Tool::Brush(b) => match m {
                Mode::Visual(_) if in_selection && in_view => self::OMNI,
                Mode::Visual(VisualState::Selecting { dragging: true }) if in_selection => {
                    self::OMNI
                }
                _ => {
                    if b.is_set(BrushMode::Erase) {
                        self::ERASE
                    } else {
                        self::CROSSHAIR
                    }
                }
            },
        };
        Some(cursor)
    }
}

mod checker {
    use rgx::core::Rect;

    pub fn rect() -> Rect<f32> {
        Rect::origin(2., 2.)
    }
}

pub struct DrawContext {
    pub ui_batch: shape2d::Batch,
    pub text_batch: TextBatch,
    pub overlay_batch: TextBatch,
    pub cursor_sprite: cursor2d::Sprite,
    pub tool_batch: sprite2d::Batch,
    pub paste_batch: sprite2d::Batch,
    pub checker_batch: sprite2d::Batch,
}

impl DrawContext {
    pub fn draw(
        &mut self,
        session: &Session,
        avg_frametime: &time::Duration,
        execution: Rc<RefCell<Execution>>,
    ) {
        self::draw_brush(&session, &mut self.ui_batch);
        self::draw_paste(&session, &mut self.paste_batch);
        self::draw_grid(&session, &mut self.ui_batch);
        self::draw_ui(&session, &mut self.ui_batch, &mut self.text_batch);
        self::draw_overlay(&session, avg_frametime, &mut self.overlay_batch, execution);
        self::draw_palette(&session, &mut self.ui_batch);
        self::draw_cursor(&session, &mut self.cursor_sprite, &mut self.tool_batch);
        self::draw_checker(&session, &mut self.checker_batch);
    }
}

fn draw_ui(session: &Session, canvas: &mut shape2d::Batch, text: &mut TextBatch) {
    let view = session.active_view();

    if let Some(selection) = session.selection {
        let fill = match session.mode {
            Mode::Visual(VisualState::Selecting { .. }) => {
                Rgba8::new(color::RED.r, color::RED.g, color::RED.b, 0x55)
            }
            // TODO: Handle different modes differently.
            _ => Rgba8::TRANSPARENT,
        };
        let stroke = color::RED;

        let r = selection.abs().bounds();
        let offset = session.offset + view.offset;

        {
            // Selection dimensions.
            let s = selection;
            let z = view.zoom;
            let t = format!("{}x{}", r.width(), r.height());
            let x = if s.x2 > s.x1 {
                (s.x2 + 1) as f32 * z - t.len() as f32 * self::GLYPH_WIDTH
            } else {
                (s.x2 as f32) * z
            };
            let y = if s.y2 >= s.y1 {
                (s.y2 + 1) as f32 * z + 1.
            } else {
                (s.y2) as f32 * z - self::LINE_HEIGHT + 1.
            };
            text.add(&t, x + offset.x, y + offset.y, self::TEXT_LAYER, stroke);
        }

        let t = Matrix4::from_translation(offset.extend(0.)) * Matrix4::from_scale(view.zoom);

        // Selection stroke.
        canvas.add(Shape::Rectangle(
            r.map(|n| n as f32).transform(t),
            self::UI_LAYER,
            Rotation::ZERO,
            Stroke::new(1., stroke.into()),
            Fill::Empty(),
        ));
        // Selection fill.
        if r.intersects(view.bounds()) {
            canvas.add(Shape::Rectangle(
                r.intersection(view.bounds()).map(|n| n as f32).transform(t),
                self::UI_LAYER,
                Rotation::ZERO,
                Stroke::NONE,
                Fill::Solid(fill.into()),
            ));
        }
    }

    for (id, v) in session.views.iter() {
        let offset = v.offset + session.offset;

        // Frame lines
        for n in 1..v.animation.len() {
            let n = n as f32;
            let x = n * v.zoom * v.fw as f32 + offset.x;
            canvas.add(Shape::Line(
                Line::new(x, offset.y, x, v.zoom * v.fh as f32 + offset.y),
                self::UI_LAYER,
                Rotation::ZERO,
                Stroke::new(1.0, Rgba::new(1., 1., 1., 0.6)),
            ));
        }
        // View border
        let r = v.rect();
        let border_color = if session.is_active(*id) {
            match session.mode {
                // TODO: (rgx) Use `Rgba8::alpha`.
                Mode::Visual(_) => {
                    Rgba8::new(color::RED.r, color::RED.g, color::RED.b, 0xdd).into()
                }
                _ => color::WHITE.into(),
            }
        } else if session.hover_view == Some(*id) {
            Rgba::new(0.7, 0.7, 0.7, 1.0)
        } else {
            Rgba::new(0.5, 0.5, 0.5, 1.0)
        };
        canvas.add(Shape::Rectangle(
            Rect::new(r.x1 - 1., r.y1 - 1., r.x2 + 1., r.y2 + 1.) + session.offset,
            self::UI_LAYER,
            Rotation::ZERO,
            Stroke::new(1.0, border_color),
            Fill::Empty(),
        ));

        if session.settings["ui/view-info"].is_set() {
            // View info
            text.add(
                &format!("{}x{}x{}", v.fw, v.fh, v.animation.len()),
                offset.x,
                offset.y - self::LINE_HEIGHT,
                self::TEXT_LAYER,
                color::GREY,
            );
        }
    }
    if session.settings["ui/status"].is_set() {
        // Active view status
        text.add(
            &view.status(),
            MARGIN,
            MARGIN + self::LINE_HEIGHT,
            self::TEXT_LAYER,
            Rgba8::WHITE,
        );

        // Session status
        text.add(
            &format!("{:>5}%", (view.zoom * 100.) as u32),
            session.width - MARGIN - 6. * 8.,
            MARGIN + self::LINE_HEIGHT,
            self::TEXT_LAYER,
            Rgba8::WHITE,
        );

        if session.width >= 600. {
            let cursor = session.view_coords(view.id, session.cursor);
            let hover_color = session
                .hover_color
                .map_or(String::new(), |c| Rgb8::from(c).to_string());
            text.add(
                &format!("{:>4},{:<4} {}", cursor.x, cursor.y, hover_color),
                session.width * 0.5,
                MARGIN + self::LINE_HEIGHT,
                self::TEXT_LAYER,
                Rgba8::WHITE,
            );
        }
    }

    if session.settings["ui/switcher"].is_set() {
        if session.width >= 400. {
            // Fg color
            canvas.add(Shape::Rectangle(
                Rect::origin(11., 11.)
                    .with_origin(session.width * 0.4, self::LINE_HEIGHT + self::MARGIN + 2.),
                self::UI_LAYER,
                Rotation::ZERO,
                Stroke::new(1.0, Rgba::WHITE),
                Fill::Solid(session.fg.into()),
            ));
            // Bg color
            canvas.add(Shape::Rectangle(
                Rect::origin(11., 11.).with_origin(
                    session.width * 0.4 + 25.,
                    self::LINE_HEIGHT + self::MARGIN + 2.,
                ),
                self::UI_LAYER,
                Rotation::ZERO,
                Stroke::new(1.0, Rgba::WHITE),
                Fill::Solid(session.bg.into()),
            ));
        }
    }

    // Command-line & message
    if session.mode == Mode::Command {
        let s = format!("{}", &session.cmdline.input());
        text.add(&s, MARGIN, MARGIN, self::TEXT_LAYER, Rgba8::WHITE);
    } else if !session.message.is_replay()
        && !session.message.is_debug()
        && session.settings["ui/message"].is_set()
    {
        let s = format!("{}", &session.message);
        text.add(
            &s,
            MARGIN,
            MARGIN,
            self::TEXT_LAYER,
            session.message.color(),
        );
    }
}

fn draw_overlay(
    session: &Session,
    avg_frametime: &time::Duration,
    text: &mut TextBatch,
    exec: Rc<RefCell<Execution>>,
) {
    let debug = session.settings["debug"].is_set();

    match &*exec.borrow() {
        Execution::Recording { path, .. } => {
            text.add(
                &format!("* recording: {} (<End> to stop)", path.display()),
                MARGIN * 2.,
                session.height - self::LINE_HEIGHT - MARGIN,
                ZDepth::ZERO,
                color::RED,
            );
        }
        Execution::Replaying { events, path, .. } => {
            if let Some(event) = events.front() {
                text.add(
                    &format!(
                        "> replaying: {}: {:32} (<Esc> to stop)",
                        path.display(),
                        String::from(event.clone()),
                    ),
                    MARGIN * 2.,
                    session.height - self::LINE_HEIGHT - MARGIN,
                    ZDepth::ZERO,
                    color::LIGHT_GREEN,
                );
            }
        }
        Execution::Normal => {}
    }

    if debug {
        let mem = crate::ALLOCATOR.allocated();

        // Frame-time
        let txt = &format!(
            "{:3.2}ms {:3.2}ms {}MB {}KB {}",
            avg_frametime.as_micros() as f64 / 1000.,
            session.avg_time.as_micros() as f64 / 1000.,
            mem / (1024 * 1024),
            mem / 1024 % (1024),
            session.mode,
        );
        text.add(
            txt,
            MARGIN,
            session.height - MARGIN - self::LINE_HEIGHT,
            ZDepth::ZERO,
            Rgba8::WHITE,
        );
    }

    if session.message.is_replay() || (session.message.is_debug() && debug) {
        text.add(
            &format!("{}", session.message),
            MARGIN,
            MARGIN,
            ZDepth::ZERO,
            session.message.color(),
        );
    }
}

fn draw_palette(session: &Session, batch: &mut shape2d::Batch) {
    if !session.settings["ui/palette"].is_set() {
        return;
    }

    let p = &session.palette;
    for (i, color) in p.colors.iter().cloned().enumerate() {
        let x = if i >= 16 { p.cellsize } else { 0. };
        let y = (i % 16) as f32 * p.cellsize;

        let mut stroke = shape2d::Stroke::NONE;
        if let (Tool::Sampler, Some(c)) = (&session.tool, p.hover) {
            if c == color {
                stroke = shape2d::Stroke::new(1., Rgba::WHITE);
            }
        }

        batch.add(Shape::Rectangle(
            Rect::new(p.x + x, p.y + y, p.x + x + p.cellsize, p.y + y + p.cellsize),
            self::PALETTE_LAYER,
            Rotation::ZERO,
            stroke,
            shape2d::Fill::Solid(color.into()),
        ));
    }
}

fn draw_checker(session: &Session, batch: &mut sprite2d::Batch) {
    if session.settings["checker"].is_set() {
        for (_, v) in session.views.iter() {
            let ratio = v.width() as f32 / v.height() as f32;
            let rx = v.zoom * ratio;
            let ry = v.zoom;

            batch.add(
                checker::rect(),
                v.rect() + session.offset,
                self::CHECKER_LAYER,
                Rgba::TRANSPARENT,
                1.,
                kit::Repeat::new(rx, ry),
            );
        }
    }
}

fn draw_grid(session: &Session, batch: &mut shape2d::Batch) {
    if session.settings["grid"].is_set() {
        let color = session.settings["grid/color"].rgba8();
        let (gx, gy) = session.settings["grid/spacing"].clone().into();

        let t = session.offset;
        let v = session.active_view();
        let w = v.width();
        let h = v.height();
        let m = Matrix4::from_translation(t.extend(0.)) * Matrix4::from_scale(v.zoom);

        // Grid columns.
        for x in (0..).step_by(gx as usize).skip(1).take_while(|x| *x < w) {
            let h = h as f32;
            let x = x as f32;

            batch.add(Shape::Line(
                Line::new(x, 0., x, h).transform(m),
                self::GRID_LAYER,
                Rotation::ZERO,
                Stroke::new(1., color.into()),
            ));
        }
        // Grid rows.
        for y in (0..).step_by(gy as usize).skip(1).take_while(|y| *y < h) {
            let w = w as f32;
            let y = y as f32;

            batch.add(Shape::Line(
                Line::new(0., y, w, y).transform(m),
                self::GRID_LAYER,
                Rotation::ZERO,
                Stroke::new(1., color.into()),
            ));
        }
    }
}

fn draw_cursor(session: &Session, inverted: &mut cursor2d::Sprite, batch: &mut sprite2d::Batch) {
    if !session.settings["ui/cursor"].is_set() {
        return;
    }
    let v = session.active_view();
    let c = session.cursor;

    if let Some(cursors::Cursor {
        rect,
        offset,
        invert,
    }) = cursors::info(
        &session.tool,
        session.mode,
        v.contains(c - session.offset),
        session.is_selected(session.view_coords(v.id, c).into()),
    ) {
        let dst = rect.with_origin(c.x, c.y) + offset;
        let zdepth = self::CURSOR_LAYER;

        if invert {
            inverted.set(rect, dst, zdepth);
        } else {
            batch.add(
                rect,
                dst,
                zdepth,
                Rgba::TRANSPARENT,
                1.,
                kit::Repeat::default(),
            );
        }
    }
}

fn draw_brush(session: &Session, shapes: &mut shape2d::Batch) {
    if session.palette.hover.is_some() {
        return;
    }
    if !session.settings["input/mouse"].is_set() {
        return;
    }
    let v = session.active_view();
    let c = session.cursor;
    let z = v.zoom;

    match session.mode {
        Mode::Visual(VisualState::Selecting { .. }) => {
            if session.is_selected(session.view_coords(v.id, c).into()) {
                return;
            }

            if v.contains(c - session.offset) {
                let c = session.snap(c, v.offset.x, v.offset.y, z);
                shapes.add(Shape::Rectangle(
                    Rect::new(c.x, c.y, c.x + z, c.y + z),
                    self::UI_LAYER,
                    Rotation::ZERO,
                    Stroke::new(1.0, color::RED.into()),
                    Fill::Empty(),
                ));
            }
        }
        Mode::Normal => {
            if let Tool::Brush(ref brush) = session.tool {
                let view_coords = session.active_view_coords(c);

                // Draw enabled brush
                if v.contains(c - session.offset) {
                    let (stroke, fill) = if brush.is_set(BrushMode::Erase) {
                        // When erasing, we draw a stroke that is the inverse of the underlying
                        // color at the cursor. Note that this isn't perfect, since it uses
                        // the current snapshot to get the color, so it may be incorrect
                        // while erasing over previously erased pixels in the same stroke.
                        // To make this 100% correct, we have to read the underlying color
                        // from the view's staging buffer.
                        if let Some(color) =
                            session.color_at(v.id, view_coords.into()).map(Rgba::from)
                        {
                            (
                                Stroke::new(
                                    1.0,
                                    Rgba::new(1. - color.r, 1. - color.g, 1. - color.b, 1.0),
                                ),
                                Fill::Empty(),
                            )
                        } else {
                            (Stroke::new(1.0, Rgba::WHITE), Fill::Empty())
                        }
                    } else {
                        (Stroke::NONE, Fill::Solid(session.fg.into()))
                    };

                    for p in brush.expand(view_coords.into(), v.extent()) {
                        shapes.add(brush.shape(
                            *session.session_coords(v.id, p.into()),
                            self::BRUSH_LAYER,
                            stroke,
                            fill,
                            v.zoom,
                            Origin::BottomLeft,
                        ));
                    }

                    // X-Ray brush mode.
                    if brush.is_set(BrushMode::XRay)
                        && brush.size == 1
                        && v.zoom >= self::XRAY_MIN_ZOOM
                    {
                        let p: ViewCoords<u32> = view_coords.into();

                        if let Some(xray) = session.color_at(v.id, p) {
                            if xray != session.fg {
                                let center = *session
                                    .session_coords(v.id, ViewCoords::new(p.x as f32, p.y as f32))
                                    + Vector2::new(z / 2., z / 2.);

                                shapes.add(Shape::Circle(
                                    center,
                                    self::BRUSH_LAYER,
                                    self::XRAY_RADIUS,
                                    16,
                                    Stroke::NONE,
                                    Fill::Solid(xray.alpha(0xff).into()),
                                ));
                            }
                        }
                    }
                // Draw disabled brush
                } else {
                    let color = if brush.is_set(BrushMode::Erase) {
                        color::GREY
                    } else {
                        session.fg
                    };
                    shapes.add(brush.shape(
                        *c,
                        self::UI_LAYER,
                        Stroke::new(1.0, color.into()),
                        Fill::Empty(),
                        v.zoom,
                        Origin::Center,
                    ));
                }
            }
        }
        _ => {}
    }
}

fn draw_paste(session: &Session, batch: &mut sprite2d::Batch) {
    if let (Mode::Visual(VisualState::Pasting), Some(s)) = (session.mode, session.selection) {
        batch.add(
            Rect::origin(batch.w as f32, batch.h as f32),
            Rect::new(s.x1 as f32, s.y1 as f32, s.x2 as f32 + 1., s.y2 as f32 + 1.),
            ZDepth::default(),
            Rgba::TRANSPARENT,
            0.9,
            kit::Repeat::default(),
        );
    }
}

pub fn draw_view_animation(session: &Session, v: &View) -> sprite2d::Batch {
    sprite2d::Batch::singleton(
        v.width(),
        v.height(),
        v.animation.val(),
        Rect::new(-(v.fw as f32), 0., 0., v.fh as f32) * v.zoom + (session.offset + v.offset),
        self::VIEW_LAYER,
        Rgba::TRANSPARENT,
        1.,
        kit::Repeat::default(),
    )
}

pub fn draw_help(session: &Session, text: &mut TextBatch, shape: &mut shape2d::Batch) {
    shape.add(Shape::Rectangle(
        Rect::origin(session.width as f32, session.height as f32),
        self::HELP_LAYER,
        Rotation::ZERO,
        Stroke::new(1., color::RED.into()),
        Fill::Solid(Rgba::BLACK),
    ));

    let column_offset = self::GLYPH_WIDTH * 16.;
    let left_margin = self::MARGIN * 2.;

    text.add(
        &format!(
            "rx v{}: help ({} to exit)",
            crate::VERSION,
            platform::Key::Escape,
        ),
        left_margin,
        session.height as f32 - self::MARGIN - self::LINE_HEIGHT,
        self::HELP_LAYER,
        color::LIGHT_GREY,
    );

    let (normal_kbs, visual_kbs): (
        Vec<(&String, &session::KeyBinding)>,
        Vec<(&String, &session::KeyBinding)>,
    ) = session
        .key_bindings
        .iter()
        .filter_map(|kb| kb.display.as_ref().map(|d| (d, kb)))
        .partition(|(_, kb)| kb.modes.contains(&Mode::Normal));

    let mut line = (0..(session.height as usize - self::LINE_HEIGHT as usize * 4))
        .rev()
        .step_by(self::LINE_HEIGHT as usize);

    for (display, kb) in normal_kbs.iter() {
        if let Some(y) = line.next() {
            text.add(display, left_margin, y as f32, self::HELP_LAYER, color::RED);
            text.add(
                &format!("{}", kb.command),
                left_margin + column_offset,
                y as f32,
                self::HELP_LAYER,
                color::LIGHT_GREY,
            );
        }
    }

    if let Some(y) = line.nth(1) {
        text.add(
            "VISUAL MODE",
            left_margin,
            y as f32,
            self::HELP_LAYER,
            color::RED,
        );
    }
    line.next();

    for (display, kb) in visual_kbs.iter() {
        if let Some(y) = line.next() {
            text.add(display, left_margin, y as f32, self::HELP_LAYER, color::RED);
            text.add(
                &format!("{}", kb.command),
                left_margin + column_offset,
                y as f32,
                self::HELP_LAYER,
                color::LIGHT_GREY,
            );
        }
    }
    for (i, l) in session::HELP.lines().enumerate() {
        let y = session.height as f32 - (i + 4) as f32 * self::LINE_HEIGHT;

        text.add(
            l,
            left_margin + column_offset * 3. + 64.,
            y,
            self::HELP_LAYER,
            color::LIGHT_GREEN,
        );
    }
}
