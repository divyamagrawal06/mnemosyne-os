//! Native ring-3 desktop: pixels and input cross the explicit kernel ABI.
use crate::sys;
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use font8x8::{BASIC_FONTS, UnicodeFonts};
use mnemosyne_abi::{
    DisplayInfo, InputEvent, SYS_DISPLAY_BLIT, SYS_DISPLAY_INFO, SYS_INPUT, TaskInfo,
};

const INK: u32 = 0x26354b;
const MUTED: u32 = 0x64758a;
const PAPER: u32 = 0xf0f3f8;
const LINE: u32 = 0xd0d9e5;
const ACCENT: u32 = 0x56558e;
const DARK: u32 = 0x1b283d;
const LIGHT: u32 = 0xe6eefa;

#[derive(Clone, Copy, PartialEq)]
enum View {
    Terminal,
    Files,
    Tasks,
    System,
}
#[derive(Clone, Copy)]
struct Rect {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}
impl Rect {
    fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x as i32
            && y >= self.y as i32
            && x < (self.x + self.w) as i32
            && y < (self.y + self.h) as i32
    }
}

pub struct Desktop {
    width: usize,
    height: usize,
    pixels: Vec<u32>,
    tile: Vec<u32>,
    view: View,
    dirty: bool,
    mouse_x: i32,
    mouse_y: i32,
    buttons: u32,
    launcher: bool,
    console: Vec<String>,
    partial: String,
    command: String,
    history: Vec<String>,
    history_index: usize,
    files: Vec<(String, usize)>,
    tasks: [TaskInfo; 4],
    ticks: u64,
    file_name: String,
    file_contents: String,
    file_read_only: bool,
    last_command: String,
    file_focus: usize,
    selected_file: Option<usize>,
    status: String,
}

impl Desktop {
    pub fn new() -> Option<Self> {
        let mut info = DisplayInfo::default();
        if sys::call(SYS_DISPLAY_INFO, &mut info as *mut _ as u64, 0, 0, 0).0 != 0 {
            return None;
        }
        let (width, height) = (info.width as usize, info.height as usize);
        // Keep allocation bounded and avoid layouts that would conceal controls.
        if width < 1024 || height < 768 || width > 1920 || height > 1200 {
            return None;
        }
        Some(Self {
            width,
            height,
            pixels: vec![0; width * height],
            tile: Vec::with_capacity(16384),
            view: View::Terminal,
            dirty: true,
            mouse_x: (width / 2) as i32,
            mouse_y: (height / 2) as i32,
            buttons: 0,
            launcher: false,
            console: vec![
                "Welcome to MnemosyneOS.".to_string(),
                String::new(),
                "Your files and task tools are on the left.".to_string(),
                "Type help for commands, ps for tasks, or ls for files.".to_string(),
                "Try: write note.txt hello from my desktop".to_string(),
                String::new(),
                "F1 Terminal   F2 Files   F3 Tasks   F4 System".to_string(),
            ],
            partial: String::new(),
            command: String::new(),
            history: Vec::new(),
            history_index: 0,
            files: Vec::new(),
            tasks: [TaskInfo::default(); 4],
            ticks: 0,
            file_name: "note.txt".to_string(),
            file_contents: String::new(),
            file_read_only: false,
            last_command: String::new(),
            file_focus: 1,
            selected_file: None,
            status: "Ready. F1 Terminal   F2 Files   F3 Tasks   F4 System".to_string(),
        })
    }

    pub fn write_console(&mut self, text: &str) {
        for character in text.chars() {
            match character {
                '\n' => {
                    self.console.push(core::mem::take(&mut self.partial));
                    if self.console.len() > 180 {
                        self.console.remove(0);
                    }
                }
                '\r' => (),
                '\x08' => {
                    self.partial.pop();
                }
                c if !c.is_control() => {
                    self.partial.push(c);
                    if self.partial.len() > 1024 {
                        self.console.push(core::mem::take(&mut self.partial));
                    }
                }
                _ => (),
            }
        }
        self.dirty = true;
    }

    pub fn update_model(
        &mut self,
        files: Vec<(String, usize)>,
        tasks: [TaskInfo; 4],
        uptime_ticks: u64,
    ) {
        if self.files != files || self.ticks / 100 != uptime_ticks / 100 {
            self.dirty = true;
        }
        self.files = files;
        self.tasks = tasks;
        self.ticks = uptime_ticks;
        if self.selected_file.is_some_and(|i| i >= self.files.len()) {
            self.selected_file = None;
        }
    }

    pub fn set_file_contents(&mut self, name: &str, contents: &str) {
        self.file_name = name.to_string();
        self.file_read_only =
            contents.len() > 420 || !contents.bytes().all(|b| (32..=126).contains(&b));
        self.file_contents = contents
            .chars()
            .take(420)
            .map(|c| {
                if c.is_ascii_graphic() || c == ' ' {
                    c
                } else {
                    '.'
                }
            })
            .collect();
        self.status = if self.file_read_only {
            "Read-only preview: file exceeds 420 bytes or contains non-printable text. Save disabled.".to_string()
        } else {
            format!(
                "Read {} from the guest disk. Tab changes editor field.",
                name
            )
        };
        self.dirty = true;
    }

    pub fn command_complete(&mut self, ok: bool, output: &str) {
        self.status = if !ok {
            let detail = output
                .lines()
                .find(|line| line.starts_with("error:"))
                .or_else(|| output.lines().find(|line| !line.is_empty()))
                .unwrap_or("Command failed. Inspect Terminal.");
            format!("Failed: {}", detail)
        } else if self.file_read_only && self.view == View::Files {
            "Read-only preview: oversized or non-printable file. Use New file to create an editable file.".to_string()
        } else if self.last_command.starts_with("write ") {
            format!(
                "Saved {} to the guest disk.",
                self.last_command
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("file")
            )
        } else {
            "Command completed. Ready.".to_string()
        };
        self.dirty = true;
    }

    /// Flush a pending frame without consuming keyboard or mouse input.
    pub fn present(&mut self) {
        if self.dirty {
            self.paint();
            self.present_rect(Rect {
                x: 0,
                y: 0,
                w: self.width,
                h: self.height,
            });
            self.dirty = false;
            self.pointer();
        }
    }

    /// Return a real shell command; the caller executes it and supplies fresh models.
    pub fn poll(&mut self) -> Option<String> {
        let mut action = None;
        let old_mouse = (self.mouse_x, self.mouse_y);
        for _ in 0..32 {
            let mut event = InputEvent::default();
            if sys::call(SYS_INPUT, &mut event as *mut _ as u64, 0, 0, 0).0 != 0 {
                break;
            }
            if event.kind == 1 {
                action = self.key(event.code);
            } else if event.kind == 2 {
                self.mouse_x = (self.mouse_x + event.x).clamp(0, self.width as i32 - 1);
                self.mouse_y = (self.mouse_y + event.y).clamp(0, self.height as i32 - 1);
                if event.code & 1 != 0 && self.buttons & 1 == 0 {
                    action = self.click();
                }
                self.buttons = event.code;
            }
            if action.is_some() {
                break;
            }
        }
        if let Some(command) = action.as_ref() {
            self.last_command = command.clone();
            self.write_console(&format!("mnemosyne> {}\n", command));
        }
        if self.dirty {
            self.paint();
            self.present_rect(Rect {
                x: 0,
                y: 0,
                w: self.width,
                h: self.height,
            });
            self.dirty = false;
            self.pointer();
        } else if old_mouse != (self.mouse_x, self.mouse_y) {
            self.present_rect(Rect {
                x: old_mouse.0 as usize,
                y: old_mouse.1 as usize,
                w: 14,
                h: 19,
            });
            self.pointer();
        }
        action
    }

    fn window(&self) -> Rect {
        Rect {
            x: 28,
            y: 75,
            w: self.width - 56,
            h: self.height - 148,
        }
    }
    fn content(&self) -> Rect {
        let r = self.window();
        Rect {
            x: r.x + 160,
            y: r.y + 50,
            w: r.w - 180,
            h: r.h - 80,
        }
    }
    fn select(&mut self, view: View) {
        self.view = view;
        self.launcher = false;
        self.dirty = true;
        let name = match view {
            View::Terminal => "terminal",
            View::Files => "files",
            View::Tasks => "tasks",
            View::System => "system",
        };
        crate::println!("{{\"event\":\"desktop-view\",\"view\":\"{}\"}}", name);
    }
    fn key(&mut self, code: u32) -> Option<String> {
        match code {
            0x110 => self.select(View::Terminal),
            0x111 => self.select(View::Files),
            0x112 => self.select(View::Tasks),
            0x113 => self.select(View::System),
            27 => {
                self.launcher = !self.launcher;
                self.dirty = true;
            }
            _ => {}
        }
        if code >= 0x110 || code == 27 {
            return None;
        }
        self.dirty = true;
        if self.view == View::Terminal {
            match code {
                13 => {
                    let command = core::mem::take(&mut self.command);
                    if !command.trim().is_empty() {
                        self.history.push(command.clone());
                        if self.history.len() > 40 {
                            self.history.remove(0);
                        }
                        self.history_index = self.history.len();
                        return Some(command);
                    }
                }
                8 => {
                    self.command.pop();
                }
                0x100 => {
                    if self.history_index > 0 {
                        self.history_index -= 1;
                        self.command = self.history[self.history_index].clone();
                    }
                }
                0x101 => {
                    self.history_index = (self.history_index + 1).min(self.history.len());
                    self.command = self
                        .history
                        .get(self.history_index)
                        .cloned()
                        .unwrap_or_default();
                }
                32..=126 if self.command.len() < 480 => {
                    self.command.push(char::from_u32(code).unwrap_or('?'))
                }
                _ => {}
            }
        } else if self.view == View::Files {
            match code {
                9 => self.file_focus = (self.file_focus + 1) % 2,
                13 => return self.save_file(),
                8 => {
                    if self.file_focus == 0 {
                        self.file_name.pop();
                    } else if !self.file_read_only {
                        self.file_contents.pop();
                    }
                }
                32..=126 => {
                    let c = char::from_u32(code).unwrap_or('?');
                    if self.file_focus == 0
                        && self.file_name.len() < 48
                        && (c.is_ascii_alphanumeric() || "._-".contains(c))
                    {
                        self.file_name.push(c);
                    } else if self.file_focus == 1
                        && !self.file_read_only
                        && self.file_contents.len() < 420
                    {
                        self.file_contents.push(c);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn save_file(&mut self) -> Option<String> {
        if self.file_read_only {
            self.status =
                "Read-only preview cannot be saved. Choose New file for an editable document."
                    .to_string();
            return None;
        }
        if self.file_name.is_empty() || !self.file_name.as_bytes()[0].is_ascii_alphanumeric() {
            self.status = "Filename must start with a letter or number.".to_string();
            return None;
        }
        if self.file_contents.is_empty() {
            self.status = "Enter file contents before saving.".to_string();
            return None;
        }
        self.status = format!("Saving {} to the guest disk...", self.file_name);
        crate::println!(
            "{{\"event\":\"desktop-file-save\",\"name\":\"{}\"}}",
            self.file_name
        );
        Some(format!("write {} {}", self.file_name, self.file_contents))
    }

    fn click(&mut self) -> Option<String> {
        let (x, y) = (self.mouse_x, self.mouse_y);
        self.dirty = true;
        if (Rect {
            x: 16,
            y: self.height - 46,
            w: 134,
            h: 34,
        })
        .contains(x, y)
        {
            self.launcher = !self.launcher;
            return None;
        }
        if self.launcher {
            let ly = self.height - 265;
            for (i, view) in [View::Terminal, View::Files, View::Tasks, View::System]
                .iter()
                .enumerate()
            {
                if (Rect {
                    x: 26,
                    y: ly + 40 + i * 36,
                    w: 225,
                    h: 32,
                })
                .contains(x, y)
                {
                    self.select(*view);
                    return None;
                }
            }
            self.launcher = false;
        }
        let window = self.window();
        for (i, view) in [View::Terminal, View::Files, View::Tasks, View::System]
            .iter()
            .enumerate()
        {
            if (Rect {
                x: window.x + 10,
                y: window.y + 60 + i * 48,
                w: 130,
                h: 38,
            })
            .contains(x, y)
            {
                self.select(*view);
                return None;
            }
            if (Rect {
                x: 170 + i * 140,
                y: self.height - 46,
                w: 130,
                h: 34,
            })
            .contains(x, y)
            {
                self.select(*view);
                return None;
            }
        }
        let c = self.content();
        if self.view == View::Files {
            let list_w = 180;
            for i in 0..self.files.len().min((c.h - 90) / 31) {
                if (Rect {
                    x: c.x,
                    y: c.y + 58 + i * 31,
                    w: list_w,
                    h: 29,
                })
                .contains(x, y)
                {
                    self.selected_file = Some(i);
                    self.file_name = self.files[i].0.clone();
                    self.file_contents.clear();
                    self.file_read_only = true;
                    self.file_focus = 1;
                    self.status = format!("Reading {}...", self.file_name);
                    crate::println!(
                        "{{\"event\":\"desktop-file-open\",\"name\":\"{}\"}}",
                        self.file_name
                    );
                    return Some(format!("cat {}", self.file_name));
                }
            }
            let ex = c.x + 200;
            if (Rect {
                x: ex,
                y: c.y + 62,
                w: c.w - 200,
                h: 34,
            })
            .contains(x, y)
            {
                self.file_focus = 0;
            }
            if (Rect {
                x: ex,
                y: c.y + 127,
                w: c.w - 200,
                h: c.h - 215,
            })
            .contains(x, y)
            {
                self.file_focus = 1;
            }
            if (Rect {
                x: ex,
                y: c.y + c.h - 60,
                w: 130,
                h: 34,
            })
            .contains(x, y)
            {
                return self.save_file();
            }
            if (Rect {
                x: ex + 140,
                y: c.y + c.h - 60,
                w: 100,
                h: 34,
            })
            .contains(x, y)
            {
                self.file_name = "new.txt".to_string();
                self.file_contents.clear();
                self.file_read_only = false;
                self.file_focus = 0;
                self.selected_file = None;
                self.status = "Type a filename, Tab to contents, then Save file.".to_string();
            }
            if (Rect {
                x: c.x + c.w - 104,
                y: c.y,
                w: 104,
                h: 30,
            })
            .contains(x, y)
            {
                return Some("ls".to_string());
            }
        }
        if self.view == View::Tasks
            && (Rect {
                x: c.x + c.w - 104,
                y: c.y,
                w: 104,
                h: 30,
            })
            .contains(x, y)
        {
            return Some("ps".to_string());
        }
        if self.view == View::Tasks {
            for (i, task) in self.tasks.iter().enumerate() {
                if task.pid > 1
                    && (Rect {
                        x: c.x + c.w - 95,
                        y: c.y + 104 + i * 48,
                        w: 95,
                        h: 28,
                    })
                    .contains(x, y)
                {
                    let operation = if task.state == 3 { "start" } else { "stop" };
                    crate::println!(
                        "{{\"event\":\"desktop-task-action\",\"action\":\"{}\",\"pid\":{}}}",
                        operation,
                        task.pid
                    );
                    return Some(format!("{} {}", operation, task.pid));
                }
            }
        }
        if self.view == View::System {
            for (i, command) in [
                "about",
                "uptime",
                "fs-info",
                "ipc-demo",
                "cap-demo",
                "fault-demo",
                "ps",
                "shutdown",
            ]
            .iter()
            .enumerate()
            {
                if (Rect {
                    x: c.x + (i % 2) * (c.w / 2),
                    y: c.y + 120 + (i / 2) * 54,
                    w: c.w / 2 - 12,
                    h: 40,
                })
                .contains(x, y)
                {
                    self.select(View::Terminal);
                    return Some((*command).to_string());
                }
            }
        }
        None
    }

    fn rect(&mut self, r: Rect, color: u32) {
        for y in r.y.min(self.height)..(r.y + r.h).min(self.height) {
            self.pixels[y * self.width + r.x.min(self.width)
                ..y * self.width + (r.x + r.w).min(self.width)]
                .fill(color);
        }
    }
    fn text(&mut self, x: usize, y: usize, text: &str, color: u32, scale: usize, max: usize) {
        for (i, ch) in text.chars().take(max).enumerate() {
            let glyph = BASIC_FONTS
                .get(ch)
                .or_else(|| BASIC_FONTS.get('?'))
                .unwrap_or([0; 8]);
            for (gy, bits) in glyph.iter().enumerate() {
                for gx in 0..8 {
                    if bits & (1 << gx) != 0 {
                        self.rect(
                            Rect {
                                x: x + i * 9 * scale + gx * scale,
                                y: y + gy * scale,
                                w: scale,
                                h: scale,
                            },
                            color,
                        );
                    }
                }
            }
        }
    }
    fn button(&mut self, r: Rect, label: &str, primary: bool) {
        self.rect(r, if primary { ACCENT } else { LINE });
        self.text(
            r.x + 12,
            r.y + (r.h - 8) / 2,
            label,
            if primary { LIGHT } else { INK },
            1,
            (r.w.saturating_sub(20)) / 9,
        );
    }

    fn paint(&mut self) {
        self.pixels.fill(0x34465d);
        // A quiet desktop, with a large geometric memory-mark rather than a wallpaper asset.
        self.rect(
            Rect {
                x: self.width - 210,
                y: 12,
                w: 170,
                h: 42,
            },
            0x3d5069,
        );
        self.text(28, 24, "Mnemosyne", LIGHT, 2, 20);
        self.text(216, 30, "OS", 0xb8b5dc, 1, 8);
        self.text(
            self.width - 192,
            29,
            &format!(
                "Uptime {:02}:{:02}",
                self.ticks / 6000,
                (self.ticks / 100) % 60
            ),
            LIGHT,
            1,
            20,
        );
        let w = self.window();
        self.rect(
            Rect {
                x: w.x + 5,
                y: w.y + 7,
                w: w.w,
                h: w.h,
            },
            0x28384d,
        );
        self.rect(w, PAPER);
        self.rect(
            Rect {
                x: w.x,
                y: w.y,
                w: w.w,
                h: 36,
            },
            0xe0e5f0,
        );
        self.text(w.x + 16, w.y + 14, "M", ACCENT, 1, 1);
        let title = match self.view {
            View::Terminal => "Terminal",
            View::Files => "Files",
            View::Tasks => "Task manager",
            View::System => "System",
        };
        self.text(w.x + 40, w.y + 14, title, INK, 1, 50);
        self.text(w.x + w.w - 193, w.y + 14, "MnemosyneOS", MUTED, 1, 21);
        self.rect(
            Rect {
                x: w.x,
                y: w.y + 36,
                w: 146,
                h: w.h - 36,
            },
            0xe7ecf4,
        );
        for (i, (label, view)) in [
            ("Terminal", View::Terminal),
            ("Files", View::Files),
            ("Tasks", View::Tasks),
            ("System", View::System),
        ]
        .iter()
        .enumerate()
        {
            let r = Rect {
                x: w.x + 10,
                y: w.y + 60 + i * 48,
                w: 130,
                h: 38,
            };
            if self.view == *view {
                self.rect(r, ACCENT);
            }
            self.text(
                r.x + 12,
                r.y + 15,
                &format!("{}  {}", i + 1, label),
                if self.view == *view { LIGHT } else { INK },
                1,
                14,
            );
        }
        self.text(w.x + 18, w.y + w.h - 60, "F1 - F4", MUTED, 1, 14);
        self.text(w.x + 18, w.y + w.h - 44, "Switch apps", MUTED, 1, 14);
        match self.view {
            View::Terminal => self.paint_terminal(),
            View::Files => self.paint_files(),
            View::Tasks => self.paint_tasks(),
            View::System => self.paint_system(),
        }
        self.rect(
            Rect {
                x: w.x + 146,
                y: w.y + w.h - 25,
                w: w.w - 146,
                h: 25,
            },
            0xe7ecf4,
        );
        let status = self.status.clone();
        self.text(
            w.x + 160,
            w.y + w.h - 16,
            &status,
            MUTED,
            1,
            (w.w - 178) / 9,
        );
        self.rect(
            Rect {
                x: 0,
                y: self.height - 58,
                w: self.width,
                h: 58,
            },
            0x26364c,
        );
        self.button(
            Rect {
                x: 16,
                y: self.height - 46,
                w: 134,
                h: 34,
            },
            "M  Launcher",
            true,
        );
        for (i, (label, view)) in [
            ("Terminal", View::Terminal),
            ("Files", View::Files),
            ("Tasks", View::Tasks),
            ("System", View::System),
        ]
        .iter()
        .enumerate()
        {
            let r = Rect {
                x: 170 + i * 140,
                y: self.height - 46,
                w: 130,
                h: 34,
            };
            if self.view == *view {
                self.rect(r, 0x40536f);
            }
            self.text(r.x + 16, r.y + 14, label, LIGHT, 1, 12);
        }
        self.text(
            self.width - 170,
            self.height - 30,
            "Local session",
            0xaab9ce,
            1,
            18,
        );
        if self.launcher {
            self.paint_launcher();
        }
    }

    fn paint_terminal(&mut self) {
        let c = self.content();
        self.rect(c, DARK);
        self.rect(
            Rect {
                x: c.x,
                y: c.y,
                w: c.w,
                h: 30,
            },
            0x27384f,
        );
        self.text(c.x + 14, c.y + 12, "Guest shell", LIGHT, 1, 30);
        self.text(
            c.x + c.w - 188,
            c.y + 12,
            "Command history",
            0xaab9ce,
            1,
            25,
        );
        let cols = (c.w - 28) / 9;
        let rows = (c.h - 93) / 15;
        let mut lines: Vec<String> = Vec::new();
        for line in self.console.iter().chain(core::iter::once(&self.partial)) {
            if line.is_empty() {
                lines.push(String::new());
                continue;
            }
            let chars: Vec<char> = line.chars().collect();
            for chunk in chars.chunks(cols.max(1)) {
                lines.push(chunk.iter().collect());
            }
        }
        let begin = lines.len().saturating_sub(rows);
        for (i, line) in lines[begin..].iter().enumerate() {
            self.text(c.x + 14, c.y + 44 + i * 15, line, LIGHT, 1, cols);
        }
        self.rect(
            Rect {
                x: c.x,
                y: c.y + c.h - 41,
                w: c.w,
                h: 41,
            },
            0x27384f,
        );
        self.text(c.x + 12, c.y + c.h - 26, ">", 0xbab5e7, 1, 1);
        let visible = self
            .command
            .chars()
            .rev()
            .take(cols.saturating_sub(4))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>();
        self.text(c.x + 30, c.y + c.h - 26, &visible, LIGHT, 1, cols - 3);
        self.rect(
            Rect {
                x: c.x + 30 + visible.len() * 9,
                y: c.y + c.h - 28,
                w: 7,
                h: 12,
            },
            0xc3bddf,
        );
    }

    fn paint_files(&mut self) {
        let c = self.content();
        self.text(c.x, c.y + 9, "Guest disk", INK, 2, 20);
        self.button(
            Rect {
                x: c.x + c.w - 104,
                y: c.y,
                w: 104,
                h: 30,
            },
            "Refresh",
            false,
        );
        self.text(
            c.x,
            c.y + 38,
            &format!("{} files", self.files.len()),
            MUTED,
            1,
            20,
        );
        self.rect(
            Rect {
                x: c.x + 184,
                y: c.y + 55,
                w: 1,
                h: c.h - 65,
            },
            LINE,
        );
        let files = self.files.clone();
        for (i, (name, bytes)) in files.iter().take((c.h - 90) / 31).enumerate() {
            let y = c.y + 58 + i * 31;
            if self.selected_file == Some(i) {
                self.rect(
                    Rect {
                        x: c.x,
                        y,
                        w: 178,
                        h: 29,
                    },
                    0xdedff0,
                );
            }
            self.text(c.x + 8, y + 5, name, INK, 1, 18);
            self.text(c.x + 8, y + 18, &format!("{} bytes", bytes), MUTED, 1, 18);
        }
        if files.is_empty() {
            self.text(c.x + 6, c.y + 70, "No guest files yet", MUTED, 1, 20);
        }
        let ex = c.x + 200;
        let ew = c.w - 200;
        self.text(ex, c.y + 43, "Filename", MUTED, 1, 20);
        self.rect(
            Rect {
                x: ex,
                y: c.y + 62,
                w: ew,
                h: 34,
            },
            if self.file_focus == 0 { ACCENT } else { LINE },
        );
        self.rect(
            Rect {
                x: ex + 2,
                y: c.y + 64,
                w: ew - 4,
                h: 30,
            },
            0xfcfdff,
        );
        let name = self.file_name.clone();
        self.text(ex + 10, c.y + 75, &name, INK, 1, (ew - 20) / 9);
        self.text(
            ex,
            c.y + 108,
            if self.file_read_only {
                "Read-only preview"
            } else {
                "File contents"
            },
            MUTED,
            1,
            24,
        );
        self.rect(
            Rect {
                x: ex,
                y: c.y + 127,
                w: ew,
                h: c.h - 215,
            },
            if self.file_focus == 1 { ACCENT } else { LINE },
        );
        self.rect(
            Rect {
                x: ex + 2,
                y: c.y + 129,
                w: ew - 4,
                h: c.h - 219,
            },
            0xfcfdff,
        );
        let contents = self.file_contents.clone();
        let cols = (ew - 20) / 9;
        let chars: Vec<char> = contents.chars().collect();
        for (i, line) in chars.chunks(cols.max(1)).take((c.h - 235) / 16).enumerate() {
            self.text(
                ex + 10,
                c.y + 140 + i * 16,
                &line.iter().collect::<String>(),
                INK,
                1,
                cols,
            );
        }
        if self.file_focus == 1 {
            let index = chars.len();
            let row = index / cols.max(1);
            if row < (c.h - 235) / 16 {
                self.rect(
                    Rect {
                        x: ex + 10 + (index % cols.max(1)) * 9,
                        y: c.y + 137 + row * 16,
                        w: 2,
                        h: 12,
                    },
                    ACCENT,
                );
            }
        }
        self.button(
            Rect {
                x: ex,
                y: c.y + c.h - 60,
                w: 130,
                h: 34,
            },
            if self.file_read_only {
                "Read only"
            } else {
                "Save file"
            },
            !self.file_read_only,
        );
        self.button(
            Rect {
                x: ex + 140,
                y: c.y + c.h - 60,
                w: 100,
                h: 34,
            },
            "New file",
            false,
        );
        self.text(
            ex,
            c.y + c.h - 14,
            if self.file_read_only {
                "Preview only. Use New file to edit."
            } else {
                "Tab: field   Enter: save"
            },
            MUTED,
            1,
            ew / 9,
        );
    }

    fn paint_tasks(&mut self) {
        let c = self.content();
        self.text(c.x, c.y + 9, "Task manager", INK, 2, 30);
        self.button(
            Rect {
                x: c.x + c.w - 104,
                y: c.y,
                w: 104,
                h: 30,
            },
            "Refresh",
            false,
        );
        self.text(c.x, c.y + 43, "Live process activity", MUTED, 1, 50);
        let columns = [0, 55, 245, 360, 475, 590];
        for (offset, label) in columns.iter().zip([
            "PID",
            "Role / state",
            "Switches",
            "Preempt",
            "Faults",
            "Epoch",
        ]) {
            self.text(c.x + offset, c.y + 84, label, MUTED, 1, 15);
        }
        let tasks = self.tasks;
        for (i, task) in tasks.iter().enumerate() {
            let y = c.y + 110 + i * 48;
            self.rect(
                Rect {
                    x: c.x,
                    y: y - 10,
                    w: c.w,
                    h: 1,
                },
                LINE,
            );
            let state = match task.state {
                0 => "ready",
                1 => "receiving",
                2 => "sleeping",
                3 => "stopped",
                _ => "other",
            };
            let role = match task.pid {
                1 => "init",
                2 => "echo",
                3 => "spinner",
                4 => "fault",
                _ => "task",
            };
            let values = [
                format!("{}", task.pid),
                format!("{} / {}", role, state),
                format!("{}", task.switches),
                format!("{}", task.preemptions),
                format!("{}", task.faults),
                format!("{}", task.epoch),
            ];
            for (offset, value) in columns.iter().zip(values.iter()) {
                self.text(c.x + offset, y, value, INK, 1, 18);
            }
            if task.pid > 1 {
                self.button(
                    Rect {
                        x: c.x + c.w - 95,
                        y: y - 6,
                        w: 95,
                        h: 28,
                    },
                    if task.state == 3 { "Start" } else { "Stop" },
                    false,
                );
            }
        }
        self.text(
            c.x,
            c.y + 330,
            "4 protected tasks. Real guest counters.",
            MUTED,
            1,
            60,
        );
        self.text(
            c.x,
            c.y + 351,
            "Timer-driven scheduling, capability-checked IPC.",
            MUTED,
            1,
            60,
        );
    }

    fn paint_system(&mut self) {
        let c = self.content();
        self.text(c.x, c.y + 9, "This is MnemosyneOS.", INK, 2, 40);
        self.text(
            c.x,
            c.y + 48,
            "Custom kernel. Native ring-3 desktop.",
            MUTED,
            1,
            60,
        );
        self.text(
            c.x,
            c.y + 68,
            "Limine framebuffer / PS2 keyboard and mouse",
            MUTED,
            1,
            60,
        );
        self.text(c.x, c.y + 88, "x86-64 guest running in QEMU", MUTED, 1, 60);
        for (i, label) in [
            "About this OS",
            "Uptime",
            "Disk information",
            "IPC demo",
            "Capabilities",
            "Fault isolation",
            "Task listing",
            "Shut down",
        ]
        .iter()
        .enumerate()
        {
            self.button(
                Rect {
                    x: c.x + (i % 2) * (c.w / 2),
                    y: c.y + 120 + (i / 2) * 54,
                    w: c.w / 2 - 12,
                    h: 40,
                },
                label,
                i == 0,
            );
        }
        self.text(
            c.x,
            c.y + 360,
            "Actions run real guest commands in Terminal.",
            MUTED,
            1,
            60,
        );
        self.text(
            c.x,
            c.y + 381,
            "Files persist on the Chronicle filesystem disk.",
            MUTED,
            1,
            60,
        );
    }

    fn paint_launcher(&mut self) {
        let y = self.height - 265;
        self.rect(
            Rect {
                x: 20,
                y,
                w: 245,
                h: 200,
            },
            PAPER,
        );
        self.rect(
            Rect {
                x: 20,
                y,
                w: 245,
                h: 30,
            },
            ACCENT,
        );
        self.text(33, y + 12, "MnemosyneOS applications", LIGHT, 1, 27);
        for (i, label) in [
            "F1  Terminal",
            "F2  Files",
            "F3  Task manager",
            "F4  System",
        ]
        .iter()
        .enumerate()
        {
            self.text(36, y + 52 + i * 36, label, INK, 1, 27);
        }
    }

    fn present_rect(&mut self, r: Rect) {
        let width = r.w.min(self.width - r.x.min(self.width));
        let height = r.h.min(self.height - r.y.min(self.height));
        if width == 0 || height == 0 {
            return;
        }
        let rows = (16384 / width).max(1);
        for offset in (0..height).step_by(rows) {
            let count = rows.min(height - offset);
            self.tile.clear();
            for y in r.y + offset..r.y + offset + count {
                self.tile.extend_from_slice(
                    &self.pixels[y * self.width + r.x..y * self.width + r.x + width],
                );
            }
            sys::call(
                SYS_DISPLAY_BLIT,
                self.tile.as_ptr() as u64,
                ((r.x as u64) << 32) | (r.y + offset) as u64,
                ((width as u64) << 32) | count as u64,
                0,
            );
        }
    }

    fn pointer(&mut self) {
        let x = self.mouse_x as usize;
        let y = self.mouse_y as usize;
        let w = 14.min(self.width - x);
        let h = 19.min(self.height - y);
        self.tile.clear();
        for dy in 0..h {
            for dx in 0..w {
                let arrow = dy < 15 && dx <= dy / 2;
                let stem = (10..19).contains(&dy) && (5..8).contains(&dx);
                let border = arrow && (dx == 0 || dx == dy / 2 || dy == 14);
                self.tile.push(if border {
                    0x152338
                } else if arrow || stem {
                    0xf7f9fd
                } else {
                    self.pixels[(y + dy) * self.width + x + dx]
                });
            }
        }
        sys::call(
            SYS_DISPLAY_BLIT,
            self.tile.as_ptr() as u64,
            ((x as u64) << 32) | y as u64,
            ((w as u64) << 32) | h as u64,
            0,
        );
    }
}
