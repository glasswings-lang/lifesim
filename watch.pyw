"""
watch.pyw -- lifesim in a window. Enter-driven, NVDA-first.

No command line. No flags. Open it, press Enter, watch a world get made.

---------------------------------------------------------------------------
NVDA notes
---------------------------------------------------------------------------
These follow the conventions already proven in Hearthkeep rather than
inventing anything new, because those were worked out the hard way.

- Narrative log is wx.TE_MULTILINE | wx.TE_READONLY. NVDA reads appended text
  automatically when the control has focus, and focus stays there so new
  events are announced without the reader chasing them.

- The log is updated with AppendText(), never SetValue(). SetValue() resets
  the caret, which makes NVDA re-read from the top.

- New passages are also pushed through nvdaController_speakText, so they are
  spoken regardless of where focus happens to be. That is the reason this is
  a wx window and not a terminal or a web page: it speaks on purpose, at a
  moment we choose, instead of hoping a screen reader notices a change.

- Speech is queued with wx.CallAfter so it happens after the UI has settled.

- Buttons are plain wx.Button so they tab cleanly. No wx.CheckBox anywhere:
  it announces as "button" to NVDA on Windows.

- Nothing refreshes underneath the reader. Text is only ever appended, and
  only when you ask for the next thing.
"""

from __future__ import annotations

import os
import queue
import random
import subprocess
import sys
import threading
import time

import wx

try:
    import nvda_speak
except Exception:                                    # pragma: no cover
    nvda_speak = None

HERE = os.path.dirname(os.path.abspath(__file__))
EXE = os.path.join(HERE, "target", "release",
                   "lifesim.exe" if os.name == "nt" else "lifesim")
READY = "<<<LIFESIM-READY"

# Outcomes worth stopping for. Most worlds never get past microbes - that is
# the honest result and the simulation should keep producing it - but somebody
# who opened this to watch life get born should not have to sit through six
# universes of pond scum to find out that is normal.
LIVELY = (
    "Bodies, but", "Animals with", "Minds,", "Tool users", "A civilisation",
)

NARRATORS = [
    ("Plain words, offline (nothing sent anywhere)", ["--narrator", "builtin"]),
    ("A model writes it, free", ["--narrator", "openrouter"]),
    ("A model on this machine", ["--narrator", "ollama"]),
]


class Engine:
    """The simulation, running as a separate program, talked to a line at a time."""

    def __init__(self, seed: str, narrator_args: list[str]) -> None:
        env = dict(os.environ)
        env["LIFESIM_GUI"] = "1"
        cmd = [EXE, "explore", "--seed", str(seed)] + narrator_args
        self.proc = subprocess.Popen(
            cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, text=True, bufsize=1, env=env,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        )
        self.out: "queue.Queue[str | None]" = queue.Queue()
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self) -> None:
        for line in self.proc.stdout:
            self.out.put(line.rstrip("\n"))
        self.out.put(None)

    def send(self, text: str) -> None:
        if self.proc.poll() is None:
            try:
                self.proc.stdin.write(text + "\n")
                self.proc.stdin.flush()
            except Exception:
                pass

    def stop(self) -> None:
        try:
            self.proc.kill()
        except Exception:
            pass


class Window(wx.Frame):
    def __init__(self) -> None:
        super().__init__(None, title="lifesim - watching a world get made",
                         size=(900, 700))
        panel = wx.Panel(self)
        root = wx.BoxSizer(wx.VERTICAL)

        # 1. The narrative. Focus lives here.
        self.log = wx.TextCtrl(
            panel, style=wx.TE_MULTILINE | wx.TE_READONLY | wx.TE_RICH2 | wx.HSCROLL)
        self.log.SetName("The story so far")
        font = self.log.GetFont()
        font.SetPointSize(font.GetPointSize() + 2)
        self.log.SetFont(font)
        root.Add(self.log, 1, wx.EXPAND | wx.ALL, 8)

        # 2. Asking it things.
        ask_row = wx.BoxSizer(wx.HORIZONTAL)
        ask_lbl = wx.StaticText(panel, label="Ask it something:")
        self.ask = wx.TextCtrl(panel, style=wx.TE_PROCESS_ENTER)
        self.ask.SetName("Ask it something")
        self.ask.SetHint("huh")
        ask_row.Add(ask_lbl, 0, wx.ALIGN_CENTER_VERTICAL | wx.RIGHT, 8)
        ask_row.Add(self.ask, 1, wx.EXPAND)
        root.Add(ask_row, 0, wx.EXPAND | wx.LEFT | wx.RIGHT | wx.BOTTOM, 8)

        # 3. Buttons, in the order you would reach for them.
        btns = wx.BoxSizer(wx.HORIZONTAL)
        self.next_btn = wx.Button(panel, label="&Next thing that happens")
        self.huh_btn = wx.Button(panel, label="&What just happened?")
        self.life_btn = wx.Button(panel, label="What is &alive")
        self.world_btn = wx.Button(panel, label="This &world")
        self.new_btn = wx.Button(panel, label="&Start a new world")
        self.more_btn = wx.Button(panel, label="&Keep going")
        self.find_btn = wx.Button(panel, label="&Find a world where something happens")
        self.tff_btn = wx.Button(panel, label="Send a creature to &Time for Family")
        self.speak_btn = wx.Button(panel, label="&Speech: on")
        for b in (self.next_btn, self.huh_btn, self.life_btn,
                  self.world_btn, self.more_btn, self.new_btn, self.find_btn,
                  self.tff_btn, self.speak_btn):
            btns.Add(b, 0, wx.RIGHT, 6)
        root.Add(btns, 0, wx.LEFT | wx.RIGHT | wx.BOTTOM, 8)

        panel.SetSizer(root)

        self.next_btn.Bind(wx.EVT_BUTTON, lambda e: self.send(""))
        self.huh_btn.Bind(wx.EVT_BUTTON, lambda e: self.send("huh"))
        self.life_btn.Bind(wx.EVT_BUTTON, lambda e: self.send("life"))
        self.world_btn.Bind(wx.EVT_BUTTON, lambda e: self.send("world"))
        self.more_btn.Bind(wx.EVT_BUTTON, lambda e: self.send("more"))
        self.new_btn.Bind(wx.EVT_BUTTON, self.on_new)
        self.find_btn.Bind(wx.EVT_BUTTON, self.on_find)
        self.tff_btn.Bind(wx.EVT_BUTTON, self.on_tff)
        self.speak_btn.Bind(wx.EVT_BUTTON, self.on_speak_toggle)
        self.ask.Bind(wx.EVT_TEXT_ENTER, self.on_ask)

        # Enter anywhere means "go on", which is the whole interaction.
        self.Bind(wx.EVT_CHAR_HOOK, self.on_key)
        self.Bind(wx.EVT_CLOSE, self.on_close)

        self.engine: Engine | None = None
        self.buffer: list[str] = []
        self.last_line_at = 0.0
        self.seed = ""
        self.narrator = NARRATORS[0][1]
        # If NVDA also reads the log automatically, this would double up. It is
        # a button rather than a setting because it needs to be one keypress
        # away the moment it turns out to be wrong.
        self.speaking = True
        self.finder: "queue.Queue[tuple] | None" = None

        self.timer = wx.Timer(self)
        self.Bind(wx.EVT_TIMER, self.on_tick, self.timer)
        self.timer.Start(60)

        self.Centre()
        self.Show()
        # A known-good world to open on. Random would be honest and would also
        # mean most people's first ever run is pond scum for six billion years.
        self.start_world("hearth")

    # -- running a world ---------------------------------------------------

    def start_world(self, seed: str) -> None:
        if self.engine:
            self.engine.stop()
        if not os.path.isfile(EXE):
            self.write("The simulation program is not built yet.\n"
                       "Open a terminal in this folder and run:  cargo build --release\n")
            return
        self.seed = seed
        self.buffer = []
        self.last_line_at = 0.0
        self.log.SetValue("")
        self.engine = Engine(seed, self.narrator)
        self.log.SetFocus()

    def on_new(self, _evt) -> None:
        dlg = wx.TextEntryDialog(
            self, "A word or a number. The same one always makes the same world.\n"
                  "Leave it empty for a world nobody has seen.",
            "Start a new world", "")
        if dlg.ShowModal() == wx.ID_OK:
            seed = dlg.GetValue().strip() or str(random.randint(1, 10 ** 12))
            self.start_world(seed)
        dlg.Destroy()

    # -- talking to it -----------------------------------------------------

    def send(self, text: str) -> None:
        if self.engine:
            self.engine.send(text)
        self.log.SetFocus()

    def on_find(self, _evt) -> None:
        if self.finder is not None:
            return
        self.finder = queue.Queue()
        self.find_btn.Disable()
        self.announce("Looking for a world where something happens. Most worlds "
                      "stay microbes forever, so this may take a few goes.")
        threading.Thread(target=self._search, args=(self.finder,),
                         daemon=True).start()

    def _search(self, out: "queue.Queue[tuple]") -> None:
        """Run whole universes quickly and keep the first interesting one.

        This is a search, not a cheat. Every universe tried is really built and
        really simulated; we are only choosing which one to sit and watch,
        the way you would pick which planet to visit.
        """
        for attempt in range(1, 25):
            seed = str(random.randint(1, 10 ** 12))
            try:
                r = subprocess.run(
                    [EXE, "run", "--seed", seed, "--detail", "brief",
                     "--narrator", "builtin"],
                    capture_output=True, text=True, timeout=180,
                    creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
            except Exception:
                continue
            outcome = ""
            for line in r.stdout.splitlines():
                if line.startswith(LIVELY):
                    outcome = line.strip()
                    break
            if outcome:
                out.put(("found", seed, outcome))
                return
            out.put(("tried", attempt, ""))
        out.put(("gave-up", 0, ""))

    def on_tff(self, _evt) -> None:
        """Send one creature over to Time for Family, without a terminal."""
        dlg = wx.TextEntryDialog(
            self, "Which creature? Leave it empty for the most interesting one.\n"
                  "Use the 'What is alive' button first if you want to pick by name.",
            "Send a creature to Time for Family", "")
        if dlg.ShowModal() != wx.ID_OK:
            dlg.Destroy()
            return
        who = dlg.GetValue().strip()
        dlg.Destroy()
        self.tff_btn.Disable()
        self.announce("Working out this world in full, then writing the creature "
                      "over. This takes a few seconds.")
        threading.Thread(target=self._to_tff, args=(who,), daemon=True).start()

    def _to_tff(self, who: str) -> None:
        world = os.path.join(HERE, "world.json")
        flags = getattr(subprocess, "CREATE_NO_WINDOW", 0)
        try:
            r = subprocess.run(
                [EXE, "run", "--seed", str(self.seed), "--narrator", "builtin",
                 "--detail", "brief", "--dump", world],
                capture_output=True, text=True, timeout=600, creationflags=flags)
            if not os.path.isfile(world):
                raise RuntimeError(r.stderr.strip() or "the world could not be written")
            cmd = [sys.executable, os.path.join(HERE, "to_tff.py"), "--force"]
            if who:
                cmd.insert(2, who)
            r2 = subprocess.run(cmd, capture_output=True, text=True,
                                timeout=120, creationflags=flags)
            msg = (r2.stdout or r2.stderr).strip() or "Done."
        except Exception as e:
            msg = f"That did not work: {e}"
        wx.CallAfter(self._tff_done, msg)

    def _tff_done(self, msg: str) -> None:
        self.tff_btn.Enable()
        self.announce(msg)

    def on_speak_toggle(self, _evt) -> None:
        self.speaking = not self.speaking
        self.speak_btn.SetLabel("&Speech: on" if self.speaking else "&Speech: off")
        if not self.speaking and nvda_speak is not None:
            nvda_speak.cancel()

    def on_ask(self, _evt) -> None:
        text = self.ask.GetValue().strip()
        self.ask.SetValue("")
        self.send(text)

    def on_key(self, evt) -> None:
        code = evt.GetKeyCode()
        focused = self.FindFocus()
        if code in (wx.WXK_RETURN, wx.WXK_NUMPAD_ENTER) and focused is not self.ask:
            self.send("")
            return
        if code == wx.WXK_ESCAPE:
            self.Close()
            return
        evt.Skip()

    # -- output ------------------------------------------------------------

    def on_tick(self, _evt) -> None:
        self.poll_finder()
        if not self.engine:
            return
        arrived = False
        while True:
            try:
                line = self.engine.out.get_nowait()
            except queue.Empty:
                break
            arrived = True
            if line is None:
                self.flush("\n(This world has finished. Start a new one whenever "
                           "you like.)\n")
                self.engine = None
                return
            if line.startswith(READY):
                self.flush()
            elif not line.strip() and self.buffer:
                # A blank line ends a passage. Flushing here is what makes the
                # text arrive a paragraph at a time and get spoken a paragraph
                # at a time, instead of the opening chapters landing as one
                # unstoppable block of speech.
                self.flush()
            elif line.strip() or self.buffer:
                self.buffer.append(line)

        # Chunk by pauses in the output rather than waiting for the next prompt.
        # The opening chapters run for thousands of lines before the first
        # prompt, and handing all of that to a screen reader in one call is an
        # unstoppable wall of speech. When the output goes quiet for a moment,
        # that is the end of a passage, so say that much and no more.
        now = time.monotonic() * 1000.0
        if arrived:
            self.last_line_at = now
        elif self.buffer and self.last_line_at and now - self.last_line_at > 250:
            self.flush()

    def poll_finder(self) -> None:
        if self.finder is None:
            return
        while True:
            try:
                kind, a, b = self.finder.get_nowait()
            except queue.Empty:
                return
            if kind == "tried":
                if a % 3 == 0:
                    self.announce(f"Still looking. {a} worlds so far, all of them "
                                  f"microbes.")
            elif kind == "found":
                self.finder = None
                self.find_btn.Enable()
                self.announce(f"Found one. {b} Starting it now.")
                self.start_world(a)
                return
            else:
                self.finder = None
                self.find_btn.Enable()
                self.announce("Could not find a lively one this time. That does "
                              "happen. Try again.")
                return

    def announce(self, text: str) -> None:
        self.write(text + "\n\n")
        if self.speaking and nvda_speak is not None:
            wx.CallAfter(nvda_speak.speak, text)

    def flush(self, extra: str = "") -> None:
        """Show and speak whatever has arrived since the last pause."""
        text = "\n".join(self.buffer).strip("\n")
        self.buffer = []
        if extra:
            text = (text + "\n" + extra) if text else extra
        if not text.strip():
            return
        self.write(text + "\n\n")
        spoken = " ".join(l.strip() for l in text.splitlines() if l.strip())
        if self.speaking and nvda_speak is not None and spoken:
            wx.CallAfter(nvda_speak.speak, spoken)

    def write(self, text: str) -> None:
        # Append only. Never SetValue on a log NVDA is reading.
        self.log.AppendText(text)
        self.log.ShowPosition(self.log.GetLastPosition())

    def on_close(self, _evt) -> None:
        self.timer.Stop()
        if self.engine:
            self.engine.stop()
        self.Destroy()


def main() -> None:
    app = wx.App(False)
    Window()
    app.MainLoop()


if __name__ == "__main__":
    main()
