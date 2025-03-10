use crate::platform::{
    ControlFlow, InputState, Key, KeyboardInput, LogicalDelta, LogicalPosition, LogicalSize,
    ModifiersState, MouseButton, WindowEvent, WindowHint,
};

use winit;
// We only support desktop platforms.
use winit::platform::desktop::EventLoopExtDesktop;

use std::io;

///////////////////////////////////////////////////////////////////////////////

pub fn run<F, T>(mut win: Window<T>, mut events: Events, mut callback: F) -> T
where
    F: 'static + FnMut(&mut Window<T>, WindowEvent) -> ControlFlow<T>,
    T: Default,
{
    let mut exit = T::default();

    win.set_cursor_visible(false);

    events
        .handle
        .run_return(|event, _, control_flow| match event {
            winit::event::Event::WindowEvent { event, .. } => {
                if let ControlFlow::Exit(r) = callback(&mut win, event.into()) {
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                    exit = r;
                }
            }
            winit::event::Event::EventsCleared => {
                if let ControlFlow::Exit(r) = callback(&mut win, WindowEvent::Ready) {
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                    exit = r;
                }
            }
            _ => {
                *control_flow = winit::event_loop::ControlFlow::Poll;
            }
        });

    exit
}

pub struct Events {
    handle: winit::event_loop::EventLoop<()>,
}

pub struct Window<T> {
    pub handle: winit::window::Window,
    phantom: std::marker::PhantomData<T>,
}

impl<T> Window<T> {
    pub fn request_redraw(&self) {
        self.handle.request_redraw();
    }

    pub fn handle(&self) -> &winit::window::Window {
        &self.handle
    }

    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.handle.set_cursor_visible(visible);
    }

    pub fn hidpi_factor(&self) -> f64 {
        self.handle.hidpi_factor()
    }

    pub fn size(&self) -> LogicalSize {
        let size = self.handle.inner_size();
        LogicalSize::new(size.width, size.height)
    }
}

pub fn init<T>(
    title: &str,
    w: u32,
    h: u32,
    hints: &[WindowHint],
) -> io::Result<(Window<T>, Events)> {
    let events = Events {
        handle: winit::event_loop::EventLoop::new(),
    };
    let mut resizable = true;
    let mut visible = true;

    for h in hints {
        match h {
            WindowHint::Resizable(r) => {
                resizable = *r;
            }
            WindowHint::Visible(v) => {
                visible = *v;
            }
        }
    }

    let handle = winit::window::WindowBuilder::new()
        .with_title(title)
        .with_inner_size(winit::dpi::LogicalSize::new(w as f64, h as f64))
        .with_resizable(resizable)
        .with_visible(visible)
        .build(&events.handle)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    Ok((
        Window {
            handle,
            phantom: std::marker::PhantomData,
        },
        events,
    ))
}

impl From<winit::dpi::LogicalSize> for LogicalSize {
    #[inline]
    fn from(size: winit::dpi::LogicalSize) -> Self {
        Self::new(size.width, size.height)
    }
}

impl From<winit::event::MouseButton> for MouseButton {
    fn from(button: winit::event::MouseButton) -> Self {
        match button {
            winit::event::MouseButton::Left => MouseButton::Left,
            winit::event::MouseButton::Right => MouseButton::Right,
            winit::event::MouseButton::Middle => MouseButton::Middle,
            winit::event::MouseButton::Other(n) => MouseButton::Other(n),
        }
    }
}

impl From<winit::event::ElementState> for InputState {
    fn from(state: winit::event::ElementState) -> Self {
        match state {
            winit::event::ElementState::Pressed => InputState::Pressed,
            winit::event::ElementState::Released => InputState::Released,
        }
    }
}

impl From<winit::event::KeyboardInput> for KeyboardInput {
    fn from(input: winit::event::KeyboardInput) -> Self {
        Self {
            state: input.state.into(),
            key: input.virtual_keycode.map(Key::from),
            modifiers: input.modifiers.into(),
        }
    }
}

impl From<winit::event::WindowEvent> for WindowEvent {
    fn from(event: winit::event::WindowEvent) -> Self {
        use winit::event::WindowEvent as Winit;

        match event {
            Winit::Resized(size) => WindowEvent::Resized(size.into()),
            Winit::Destroyed => WindowEvent::Destroyed,
            Winit::CloseRequested => WindowEvent::CloseRequested,
            Winit::RedrawRequested => WindowEvent::RedrawRequested,
            Winit::Moved(pos) => WindowEvent::Moved(pos.into()),
            Winit::MouseInput {
                state,
                button,
                modifiers,
                ..
            } => WindowEvent::MouseInput {
                state: state.into(),
                button: button.into(),
                modifiers: modifiers.into(),
            },
            Winit::MouseWheel { delta, .. } => WindowEvent::MouseWheel {
                delta: delta.into(),
            },
            Winit::CursorLeft { .. } => WindowEvent::CursorLeft,
            Winit::CursorEntered { .. } => WindowEvent::CursorEntered,
            Winit::CursorMoved { position, .. } => WindowEvent::CursorMoved {
                position: position.into(),
            },
            Winit::ReceivedCharacter(c) => WindowEvent::ReceivedCharacter(c),
            Winit::KeyboardInput { input, .. } => WindowEvent::KeyboardInput(input.into()),
            Winit::Focused(b) => WindowEvent::Focused(b),
            Winit::HiDpiFactorChanged(n) => WindowEvent::HiDpiFactorChanged(n),

            _ => WindowEvent::Noop,
        }
    }
}

impl From<winit::event::VirtualKeyCode> for Key {
    fn from(k: winit::event::VirtualKeyCode) -> Self {
        use winit::event::VirtualKeyCode as Winit;

        match k {
            Winit::Key1 => Key::Num1,
            Winit::Key2 => Key::Num2,
            Winit::Key3 => Key::Num3,
            Winit::Key4 => Key::Num4,
            Winit::Key5 => Key::Num5,
            Winit::Key6 => Key::Num6,
            Winit::Key7 => Key::Num7,
            Winit::Key8 => Key::Num8,
            Winit::Key9 => Key::Num9,
            Winit::Key0 => Key::Num0,
            Winit::A => Key::A,
            Winit::B => Key::B,
            Winit::C => Key::C,
            Winit::D => Key::D,
            Winit::E => Key::E,
            Winit::F => Key::F,
            Winit::G => Key::G,
            Winit::H => Key::H,
            Winit::I => Key::I,
            Winit::J => Key::J,
            Winit::K => Key::K,
            Winit::L => Key::L,
            Winit::M => Key::M,
            Winit::N => Key::N,
            Winit::O => Key::O,
            Winit::P => Key::P,
            Winit::Q => Key::Q,
            Winit::R => Key::R,
            Winit::S => Key::S,
            Winit::T => Key::T,
            Winit::U => Key::U,
            Winit::V => Key::V,
            Winit::W => Key::W,
            Winit::X => Key::X,
            Winit::Y => Key::Y,
            Winit::Z => Key::Z,
            Winit::Escape => Key::Escape,
            Winit::Insert => Key::Insert,
            Winit::Home => Key::Home,
            Winit::Delete => Key::Delete,
            Winit::End => Key::End,
            Winit::PageDown => Key::PageDown,
            Winit::PageUp => Key::PageUp,
            Winit::Left => Key::Left,
            Winit::Up => Key::Up,
            Winit::Right => Key::Right,
            Winit::Down => Key::Down,
            Winit::Back => Key::Backspace,
            Winit::Return => Key::Return,
            Winit::Space => Key::Space,
            Winit::Caret => Key::Caret,
            Winit::Apostrophe => Key::Apostrophe,
            Winit::Backslash => Key::Backslash,
            Winit::Colon => Key::Colon,
            Winit::Comma => Key::Comma,
            Winit::Equals => Key::Equal,
            Winit::Grave => Key::Grave,
            Winit::LAlt => Key::Alt,
            Winit::LBracket => Key::LBracket,
            Winit::LControl => Key::Control,
            Winit::LShift => Key::Shift,
            Winit::Subtract => Key::Minus,
            Winit::Period => Key::Period,
            Winit::RAlt => Key::Alt,
            Winit::RBracket => Key::RBracket,
            Winit::RControl => Key::Control,
            Winit::RShift => Key::Shift,
            Winit::Semicolon => Key::Semicolon,
            Winit::Slash => Key::Slash,
            Winit::Tab => Key::Tab,
            _ => Key::Unknown,
        }
    }
}

impl From<winit::event::ModifiersState> for ModifiersState {
    fn from(mods: winit::event::ModifiersState) -> Self {
        Self {
            shift: mods.shift,
            ctrl: mods.ctrl,
            alt: mods.alt,
            meta: mods.logo,
        }
    }
}

impl From<winit::dpi::LogicalPosition> for LogicalPosition {
    fn from(pos: winit::dpi::LogicalPosition) -> Self {
        Self { x: pos.x, y: pos.y }
    }
}

impl From<winit::event::MouseScrollDelta> for LogicalDelta {
    fn from(delta: winit::event::MouseScrollDelta) -> Self {
        match delta {
            winit::event::MouseScrollDelta::LineDelta(x, y) => LogicalDelta {
                x: x as f64,
                y: y as f64,
            },
            winit::event::MouseScrollDelta::PixelDelta(pos) => LogicalDelta { x: pos.x, y: pos.y },
        }
    }
}
