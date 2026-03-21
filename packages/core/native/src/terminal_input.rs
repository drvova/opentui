use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton,
    MouseEvent, MouseEventKind,
};

#[derive(Debug)]
pub struct TerminalInputBridge {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    receiver: Receiver<QueuedInputEvent>,
}

#[derive(Debug)]
pub struct QueuedInputEvent {
    pub name: &'static str,
    pub payload: Vec<u8>,
}

impl TerminalInputBridge {
    pub fn start(renderer_ptr: u64) -> Self {
        let (sender, receiver) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);

        let handle = thread::spawn(move || run_input_loop(renderer_ptr, thread_stop, sender));

        Self {
            stop,
            handle: Some(handle),
            receiver,
        }
    }

    pub fn drain(&self, out: &mut VecDeque<QueuedInputEvent>) {
        for event in self.receiver.try_iter() {
            out.push_back(event);
        }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for TerminalInputBridge {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_input_loop(renderer_ptr: u64, stop: Arc<AtomicBool>, sender: Sender<QueuedInputEvent>) {
    while !stop.load(Ordering::Relaxed) {
        match event::poll(Duration::from_millis(16)) {
            Ok(true) => match event::read() {
                Ok(event) => {
                    if let Some(message) = encode_event(renderer_ptr, event) {
                        let _ = sender.send(message);
                    }
                }
                Err(_) => {}
            },
            Ok(false) => {}
            Err(_) => {}
        }
    }
}

fn encode_event(renderer_ptr: u64, event: Event) -> Option<QueuedInputEvent> {
    match event {
        Event::Key(key) => Some(QueuedInputEvent {
            name: "terminal:key",
            payload: encode_key_event(renderer_ptr, key),
        }),
        Event::Mouse(mouse) => Some(QueuedInputEvent {
            name: "terminal:mouse",
            payload: encode_mouse_event(renderer_ptr, mouse),
        }),
        Event::Paste(text) => Some(QueuedInputEvent {
            name: "terminal:paste",
            payload: format!(
                "{{\"renderer\":{renderer_ptr},\"text\":{}}}",
                json_string(&text)
            )
            .into_bytes(),
        }),
        Event::FocusGained => Some(QueuedInputEvent {
            name: "terminal:focus",
            payload: format!("{{\"renderer\":{renderer_ptr},\"focused\":true}}").into_bytes(),
        }),
        Event::FocusLost => Some(QueuedInputEvent {
            name: "terminal:focus",
            payload: format!("{{\"renderer\":{renderer_ptr},\"focused\":false}}").into_bytes(),
        }),
        _ => None,
    }
}

fn encode_key_event(renderer_ptr: u64, key: KeyEvent) -> Vec<u8> {
    let modifiers = decode_modifiers(key.modifiers);
    let event_type = match key.kind {
        KeyEventKind::Press => "press",
        KeyEventKind::Repeat => "repeat",
        KeyEventKind::Release => "release",
    };
    let repeated = matches!(key.kind, KeyEventKind::Repeat);
    let sequence = key_sequence(&key);
    let raw = sequence.clone();
    let name = key_name(&key);
    let number = matches!(key.code, KeyCode::Char(ch) if ch.is_ascii_digit());
    let caps_lock = key.state.contains(KeyEventState::CAPS_LOCK);
    let num_lock = key.state.contains(KeyEventState::NUM_LOCK);

    format!(
        concat!(
            "{{\"renderer\":{renderer},\"name\":{name},\"ctrl\":{ctrl},\"meta\":{meta},\"shift\":{shift},",
            "\"option\":{option},\"sequence\":{sequence},\"number\":{number},\"raw\":{raw},",
            "\"eventType\":{event_type},\"source\":\"kitty\",\"super\":{super_mod},\"hyper\":{hyper},",
            "\"capsLock\":{caps_lock},\"numLock\":{num_lock},\"repeated\":{repeated}}}"
        ),
        renderer = renderer_ptr,
        name = json_string(&name),
        ctrl = modifiers.ctrl,
        meta = modifiers.alt,
        shift = modifiers.shift,
        option = modifiers.alt,
        sequence = json_string(&sequence),
        number = number,
        raw = json_string(&raw),
        event_type = json_string(event_type),
        super_mod = modifiers.super_mod,
        hyper = modifiers.hyper,
        caps_lock = caps_lock,
        num_lock = num_lock,
        repeated = repeated,
    )
    .into_bytes()
}

fn encode_mouse_event(renderer_ptr: u64, mouse: MouseEvent) -> Vec<u8> {
    let modifiers = decode_modifiers(mouse.modifiers);
    let (event_type, button, scroll) = decode_mouse(mouse.kind);
    let scroll_json = scroll.unwrap_or_else(|| String::from("null"));

    format!(
        concat!(
            "{{\"renderer\":{renderer},\"type\":{event_type},\"button\":{button},\"x\":{x},\"y\":{y},",
            "\"modifiers\":{{\"shift\":{shift},\"alt\":{alt},\"ctrl\":{ctrl}}},\"scroll\":{scroll}}}"
        ),
        renderer = renderer_ptr,
        event_type = json_string(event_type),
        button = button,
        x = mouse.column,
        y = mouse.row,
        shift = modifiers.shift,
        alt = modifiers.alt,
        ctrl = modifiers.ctrl,
        scroll = scroll_json,
    )
    .into_bytes()
}

struct NativeModifiers {
    shift: bool,
    alt: bool,
    ctrl: bool,
    super_mod: bool,
    hyper: bool,
}

fn decode_modifiers(modifiers: KeyModifiers) -> NativeModifiers {
    NativeModifiers {
        shift: modifiers.contains(KeyModifiers::SHIFT),
        alt: modifiers.contains(KeyModifiers::ALT),
        ctrl: modifiers.contains(KeyModifiers::CONTROL),
        super_mod: modifiers.contains(KeyModifiers::SUPER),
        hyper: modifiers.contains(KeyModifiers::HYPER),
    }
}

fn key_name(key: &KeyEvent) -> String {
    match key.code {
        KeyCode::Backspace => String::from("backspace"),
        KeyCode::Enter => String::from("return"),
        KeyCode::Left => String::from("left"),
        KeyCode::Right => String::from("right"),
        KeyCode::Up => String::from("up"),
        KeyCode::Down => String::from("down"),
        KeyCode::Home => String::from("home"),
        KeyCode::End => String::from("end"),
        KeyCode::PageUp => String::from("pageup"),
        KeyCode::PageDown => String::from("pagedown"),
        KeyCode::Tab | KeyCode::BackTab => String::from("tab"),
        KeyCode::Delete => String::from("delete"),
        KeyCode::Insert => String::from("insert"),
        KeyCode::Esc => String::from("escape"),
        KeyCode::F(number) => format!("f{number}"),
        KeyCode::Char(ch) => {
            if ch.is_ascii_uppercase() {
                ch.to_ascii_lowercase().to_string()
            } else {
                ch.to_string()
            }
        }
        _ => String::new(),
    }
}

fn key_sequence(key: &KeyEvent) -> String {
    match key.code {
        KeyCode::Enter => String::from("\r"),
        KeyCode::Tab => String::from("\t"),
        KeyCode::BackTab => String::from("\x1b[Z"),
        KeyCode::Backspace => String::from("\u{7f}"),
        KeyCode::Esc => String::from("\x1b"),
        KeyCode::Left => String::from("\x1b[D"),
        KeyCode::Right => String::from("\x1b[C"),
        KeyCode::Up => String::from("\x1b[A"),
        KeyCode::Down => String::from("\x1b[B"),
        KeyCode::Home => String::from("\x1b[H"),
        KeyCode::End => String::from("\x1b[F"),
        KeyCode::PageUp => String::from("\x1b[5~"),
        KeyCode::PageDown => String::from("\x1b[6~"),
        KeyCode::Delete => String::from("\x1b[3~"),
        KeyCode::Insert => String::from("\x1b[2~"),
        KeyCode::F(number) => format!("<f{number}>"),
        KeyCode::Char(ch) => ch.to_string(),
        _ => String::new(),
    }
}

fn decode_mouse(kind: MouseEventKind) -> (&'static str, u8, Option<String>) {
    match kind {
        MouseEventKind::Down(button) => ("down", mouse_button(button), None),
        MouseEventKind::Up(button) => ("up", mouse_button(button), None),
        MouseEventKind::Drag(button) => ("drag", mouse_button(button), None),
        MouseEventKind::Moved => ("move", 0, None),
        MouseEventKind::ScrollDown => (
            "scroll",
            0,
            Some(String::from("{\"direction\":\"down\",\"delta\":1}")),
        ),
        MouseEventKind::ScrollUp => (
            "scroll",
            0,
            Some(String::from("{\"direction\":\"up\",\"delta\":1}")),
        ),
        MouseEventKind::ScrollLeft => (
            "scroll",
            0,
            Some(String::from("{\"direction\":\"left\",\"delta\":1}")),
        ),
        MouseEventKind::ScrollRight => (
            "scroll",
            0,
            Some(String::from("{\"direction\":\"right\",\"delta\":1}")),
        ),
    }
}

fn mouse_button(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
    }
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
