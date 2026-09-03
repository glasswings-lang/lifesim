"""
nvda_speak.py — Hearthkeep NVDA announcement helper.

Calls nvdaController_speakText() via the NVDA Controller Client DLL so that
new game events are spoken immediately, regardless of where keyboard focus is.

The DLL is NOT included in NVDA's install directory — it must be downloaded
separately and placed next to this file (or in the same folder as the game).

Download from:
  https://github.com/nvaccess/nvda/releases
  → look for nvdaControllerClient_*.zip in the assets of any recent release.

Extract either:
  nvdaControllerClient64.dll  (for 64-bit Python — most common)
  nvdaControllerClient32.dll  (for 32-bit Python)

Rename the appropriate one to: nvdaControllerClient.dll
Place it in the same folder as main_menu.py / game_screen.py.

If the DLL is absent or NVDA is not running, speak() silently does nothing.
The game is fully usable without it; this is purely an accessibility layer.
"""

import ctypes
import os
import struct
import sys

_nvda = None        # loaded DLL or None
_speak_fn = None    # bound function or None
_initialised = False


def _init():
    global _nvda, _speak_fn, _initialised
    if _initialised:
        return
    _initialised = True

    # Locate the DLL next to this file or in the current working directory
    here = os.path.dirname(os.path.abspath(__file__))
    # Try both the renamed generic name and the original architecture-specific names
    dll_names = [
        "nvdaControllerClient.dll",
        "nvdaControllerClient64.dll",
        "nvdaControllerClient32.dll",
    ]
    candidates = [
        os.path.join(base, name)
        for base in (here, os.getcwd())
        for name in dll_names
    ]

    dll_path = next((p for p in candidates if os.path.isfile(p)), None)
    if dll_path is None:
        return  # DLL not present; speak() will be a no-op

    try:
        _nvda = ctypes.windll.LoadLibrary(dll_path)
        fn = _nvda.nvdaController_speakText
        fn.restype  = ctypes.c_long
        fn.argtypes = [ctypes.c_wchar_p]
        _speak_fn = fn
    except Exception:
        _nvda = None
        _speak_fn = None


def speak(text: str) -> None:
    """
    Queue text to be spoken through NVDA.
    Does not cancel current speech — let the game_screen speech queue
    manage pacing so focus changes do not interrupt mid-sentence.
    """
    _init()
    if _speak_fn is None:
        return
    try:
        _speak_fn(text)
    except Exception:
        pass  # Never crash the game over a speech failure


def cancel() -> None:
    """Stop any NVDA speech in progress."""
    _init()
    if _nvda is None:
        return
    try:
        cancel_fn = getattr(_nvda, "nvdaController_cancelSpeech", None)
        if cancel_fn:
            cancel_fn()
    except Exception:
        pass


def is_available() -> bool:
    """Return True if NVDA is running and the DLL loaded successfully."""
    _init()
    return _speak_fn is not None


if __name__ == "__main__":
    # Quick test: run this file directly to check if NVDA speech works.
    if is_available():
        speak("Hearthkeep NVDA speech test. If you can hear this, it is working.")
        print("Spoke to NVDA successfully.")
    else:
        print(
            "NVDA controller DLL not found or NVDA not running.\n"
            "Place nvdaControllerClient.dll next to this file to enable speech."
        )
