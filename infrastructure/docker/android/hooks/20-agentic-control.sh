#!/usr/bin/env bash
# F32: Agentic control — Android emulator.
# ADB provides keyboard, tap, and screenshot. QEMU hooks are inherited
# from the base but the emulator manages its own QEMU internally.

echo "[agentic] ADB input:  adb shell input keyevent KEYCODE_HOME"
echo "[agentic] ADB tap:    adb shell input tap 500 500"
echo "[agentic] ADB text:   adb shell input text 'hello'"
echo "[agentic] Screenshot: adb exec-out screencap -p > screen.png"
echo "[agentic] VNC:        vnc://localhost:5900"
echo "[agentic] noVNC:      http://localhost:6080/vnc.html"

return 0
