//! Windows toast notifications, so a long run can be left alone.
//!
//! A universe takes minutes to narrate and hours to watch at a readable pace.
//! The point of this module is that you can put the window behind something
//! else and still be told when the oxygen arrives.
//!
//! Two constraints shaped it:
//!
//! * A screen reader announces toasts. That makes them genuinely useful here
//!   and also makes flooding them actively hostile, so only real events raise
//!   one, never the routine status lines, and there is a hard rate limit with
//!   dropping rather than queueing. A notification you cannot keep up with is
//!   worse than none.
//! * No crates. The toast goes out through PowerShell's WinRT bindings, and
//!   the text is base64-encoded on the way so that quotes, apostrophes and
//!   anything else in the prose cannot break the command or be interpreted by
//!   the shell.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct Toaster {
    pub enabled: bool,
    /// Shortest gap between two notifications. Anything that arrives sooner is
    /// dropped, not held: by the time a queued one appeared it would be
    /// describing something that scrolled past long ago.
    pub min_gap: Duration,
    last: Option<Instant>,
    pub sent: u32,
    /// Events that arrived inside the rate limit and were not shown. The next
    /// notification says how many, because silently swallowing them would let
    /// someone believe they had seen everything.
    skipped: u32,
}

impl Toaster {
    pub fn new(enabled: bool) -> Toaster {
        Toaster { enabled, min_gap: Duration::from_secs(4), last: None, sent: 0, skipped: 0 }
    }

    pub fn notify(&mut self, title: &str, body: &str) {
        if !self.enabled { return; }
        if let Some(t) = self.last {
            if t.elapsed() < self.min_gap {
                self.skipped += 1;
                return;
            }
        }
        let body = if self.skipped > 0 {
            let n = self.skipped;
            self.skipped = 0;
            format!("{} (and {} other event{} just before this one)",
                trim_to(body, 170), n, if n == 1 { "" } else { "s" })
        } else {
            trim_to(body, 220)
        };
        self.last = Some(Instant::now());
        self.sent += 1;
        let _ = show(title, &body);
    }
}

fn trim_to(s: &str, n: usize) -> String {
    let clean: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.chars().count() <= n { return clean; }
    let cut: String = clean.chars().take(n).collect();
    match cut.rfind(' ') {
        Some(i) => format!("{}...", &cut[..i]),
        None => cut,
    }
}

#[cfg(windows)]
fn show(title: &str, body: &str) -> std::io::Result<()> {
    // The application identifier decides which app the notification appears to
    // come from, and Windows will silently drop a toast from an identifier it
    // does not know. PowerShell's own registered identifier is used because it
    // is always present, and because registering a new one would mean writing
    // to the Start Menu, which is not a thing a simulation should do to
    // somebody's computer.
    const AUMID: &str =
        "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe";

    let script = format!(
        "$ErrorActionPreference='SilentlyContinue';\
         [void][Windows.UI.Notifications.ToastNotificationManager,Windows.UI.Notifications,ContentType=WindowsRuntime];\
         [void][Windows.Data.Xml.Dom.XmlDocument,Windows.Data.Xml.Dom,ContentType=WindowsRuntime];\
         $t=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{}'));\
         $b=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{}'));\
         $x=[Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02);\
         $n=$x.GetElementsByTagName('text');\
         [void]$n.Item(0).AppendChild($x.CreateTextNode($t));\
         [void]$n.Item(1).AppendChild($x.CreateTextNode($b));\
         $toast=[Windows.UI.Notifications.ToastNotification]::new($x);\
         [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{}').Show($toast);",
        b64(title.as_bytes()), b64(body.as_bytes()), AUMID);

    // Spawned and not waited on. A toast must never hold up the simulation.
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script])
        .stdout(Stdio::null()).stderr(Stdio::null()).stdin(Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(not(windows))]
fn show(_title: &str, _body: &str) -> std::io::Result<()> {
    // Elsewhere, quietly do nothing rather than pretending.
    Ok(())
}

/// Standard base64. Small enough to carry rather than take a dependency for.
fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}
