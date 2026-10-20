#!/usr/bin/env python3
"""Sail the browser build from a script, and write down what it says about itself.

    dist/tab.py --seconds 40 --frames            # the published tab, timed
    dist/tab.py --url http://localhost:8777/ --shot /tmp/tab.png
    dist/tab.py --frames --profile               # and where the time goes

**Everything else in this project is measured, and the tab was not.** The sea's cost
on a desk is four compute probes and a screenshot away; in a browser the same question
had no answer at all, because the page needs a click to start and a plain
`--screenshot` catches the loading card. So the measurements that mattered most — the
tab runs at forty-eight frames a second, two milliseconds of them ours — were a number
read off a screen by eye, once, and never again.

This drives it instead: headless Chrome over the DevTools protocol, which can press
the start button, hold keys down, read the game's own log lines out of the console,
sample the stack while it sails, and photograph the result. Chrome rather than a
browser-automation library, because a library is a dependency and a WebSocket is not.

Needs Chrome (for WebGPU: Safari's headless mode has none) and `websocket-client`.
"""

import argparse
import base64
import collections
import json
import re
import subprocess
import sys
import time
import urllib.request

try:
    import websocket  # websocket-client
except ImportError:  # pragma: no cover - a message rather than a traceback
    sys.exit("dist/tab.py: pip install websocket-client")

CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
PUBLISHED = "https://kapoost.github.io/sail/play/"
PORT = 9333


class Tab:
    """One headless page, and the wire to it."""

    def __init__(self, url, size, port=PORT):
        self.chrome = subprocess.Popen(
            [
                CHROME,
                "--headless=new",
                # WebGPU is the whole reason this is Chrome and not something
                # lighter: the sea reads its spectrum out of a storage buffer.
                "--enable-unsafe-webgpu",
                "--use-angle=metal",
                f"--remote-debugging-port={port}",
                f"--window-size={size[0]},{size[1]}",
                "--remote-allow-origins=*",
                "--no-first-run",
                "--user-data-dir=/tmp/sail-tab-profile",
                url,
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        page = None
        for _ in range(80):
            try:
                listing = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json"))
                page = next(
                    t for t in listing if t["type"] == "page" and "about:blank" not in t["url"]
                )
                break
            except Exception:
                time.sleep(0.5)
        if page is None:
            self.close()
            sys.exit("dist/tab.py: chrome never opened the page")
        self.ws = websocket.create_connection(page["webSocketDebuggerUrl"], timeout=180)
        self.n = 0
        self.console = []

    # ---- the protocol ----------------------------------------------------

    def send(self, method, **params):
        """One command, and the events that arrive before its answer are kept."""
        self.n += 1
        self.ws.send(json.dumps({"id": self.n, "method": method, "params": params}))
        while True:
            message = json.loads(self.ws.recv())
            if message.get("id") == self.n:
                return message.get("result", {})
            self.note(message)

    def note(self, message):
        if message.get("method") == "Runtime.consoleAPICalled":
            args = message["params"].get("args", [])
            parts = [str(a.get("value", a.get("description", ""))) for a in args]
            # Bevy's web logger styles its lines, so the first argument carries the
            # text with `%c` markers in it and the rest are CSS. Keeping the CSS
            # would triple the length of every line for no reading at all.
            text = " ".join(p for p in parts if "color:" not in p)
            self.console.append(text.replace("%c", " ").strip())

    def pump(self, seconds):
        """Let it sail, keeping whatever it says."""
        self.ws.settimeout(0.5)
        until = time.time() + seconds
        while time.time() < until:
            try:
                self.note(json.loads(self.ws.recv()))
            except Exception:
                pass
        self.ws.settimeout(180)

    def js(self, expression):
        answer = self.send(
            "Runtime.evaluate", expression=expression, returnByValue=True, awaitPromise=True
        )
        return answer.get("result", {}).get("value")

    # ---- the game --------------------------------------------------------

    def start(self, patience=240):
        """Wait for the button to come alive, then press it.

        The button is disabled until the wasm is *compiled*, not merely fetched, so
        this is also the honest measure of how long the card is up.
        """
        self.send("Runtime.enable")
        self.send("Page.enable")
        began = time.time()
        state = "none"
        for _ in range(patience):
            state = self.js(
                "(() => {const b = document.querySelector('button');"
                " return b ? (b.disabled ? 'waiting' : 'ready') : 'none';})()"
            )
            if state == "ready":
                break
            time.sleep(1.0)
        if state != "ready":
            self.close()
            sys.exit("dist/tab.py: the start button never came alive")
        self.js("document.querySelector('button').click()")
        return time.time() - began

    def press(self, keys, hold=0.08, between=1.5):
        """Keys by their names on a chart rather than their codes."""
        named = {
            "]": ("BracketRight", 221),
            "[": ("BracketLeft", 219),
            "esc": ("Escape", 27),
            "space": ("Space", 32),
        }
        for key in keys:
            code, vk = named.get(key.lower(), (f"Key{key.upper()}", ord(key.upper()[:1])))
            for kind in ("keyDown", "keyUp"):
                self.send(
                    "Input.dispatchKeyEvent",
                    type=kind,
                    key=key,
                    code=code,
                    windowsVirtualKeyCode=vk,
                    nativeVirtualKeyCode=vk,
                )
                time.sleep(hold)
            time.sleep(between)

    def shot(self, path):
        picture = self.send("Page.captureScreenshot", format="png")
        with open(path, "wb") as f:
            f.write(base64.b64decode(picture["data"]))

    def close(self):
        try:
            self.ws.close()
        except Exception:
            pass
        self.chrome.terminate()


def wasm_names(path):
    """Function index to name, read out of the module's own name section.

    **Chrome's sampler answers `wasm-function[18521]` and means it.** The names are in
    the module — a build made with `dist/web.sh --named` keeps them — but they are
    resolved by the DevTools *frontend*, which is exactly the thing a headless run does
    not have. So the mapping is done here, from the same file the page just fetched,
    and a profile stops being a list of numbers.
    """

    def leb(data, i):
        value = shift = 0
        while True:
            byte = data[i]
            i += 1
            value |= (byte & 0x7F) << shift
            shift += 7
            if not byte & 0x80:
                return value, i

    blob = open(path, "rb").read()
    if blob[:4] != b"\x00asm":
        return {}
    i = 8
    names = {}
    while i < len(blob):
        section = blob[i]
        i += 1
        size, i = leb(blob, i)
        end = i + size
        if section == 0:
            length, j = leb(blob, i)
            if blob[j : j + length] == b"name":
                j += length
                while j < end:
                    kind = blob[j]
                    j += 1
                    part, j = leb(blob, j)
                    after = j + part
                    if kind == 1:  # the function names
                        count, j = leb(blob, j)
                        for _ in range(count):
                            index, j = leb(blob, j)
                            length, j = leb(blob, j)
                            names[index] = blob[j : j + length].decode("utf8", "replace")
                            j += length
                    j = after
        i = end
    return names


def profile(tab, seconds, names=None):
    """Sample the stack while she sails, and say what the top of it was.

    A wasm build keeps its name section unless it is stripped, so the frames come back
    as real function names — which is the one thing a frame-rate number cannot tell
    you: *whose* milliseconds those are.
    """
    tab.send("Profiler.enable")
    tab.send("Profiler.setSamplingInterval", interval=200)
    tab.send("Profiler.start")
    tab.pump(seconds)
    taken = tab.send("Profiler.stop")["profile"]
    by_id = {node["id"]: node for node in taken["nodes"]}
    # Self time: how many samples landed *in* each function rather than under it.
    tally = collections.Counter()
    for node in taken["nodes"]:
        frame = node["callFrame"]
        name = frame.get("functionName") or "(anonymous)"
        tally[name] += node.get("hitCount", 0)
    total = sum(tally.values()) or 1
    look = re.compile(r"wasm-function\[(\d+)\]")
    out = []
    for name, hits in tally.most_common(14):
        found = look.fullmatch(name)
        if found and names:
            name = names.get(int(found.group(1)), name)
        out.append((name, hits / total))
    return out, by_id


def summarise(lines):
    """The game's own two reports, averaged over however many it printed."""
    fps, worst, sea, foam, cloud = [], [], [], [], []
    for line in lines:
        slow = re.search(r"slow frames: ([\d.]+) per second mean, worst ([\d.]+) ms", line)
        if slow:
            fps.append(float(slow.group(1)))
            worst.append(float(slow.group(2)))
        spent = re.search(
            r"frame: sea ([\d.]+) ms, foam ([\d.]+) ms, cloud ([\d.]+) ms", line
        )
        if spent:
            sea.append(float(spent.group(1)))
            foam.append(float(spent.group(2)))
            cloud.append(float(spent.group(3)))
    mean = lambda v: sum(v) / len(v) if v else float("nan")  # noqa: E731
    return {
        "fps": mean(fps),
        "worst": max(worst) if worst else float("nan"),
        "sea": mean(sea),
        "foam": mean(foam),
        "cloud": mean(cloud),
        "reports": len(fps),
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--url", default=PUBLISHED)
    ap.add_argument("--size", default="1440x900")
    ap.add_argument("--seconds", type=float, default=30.0)
    ap.add_argument("--keys", default="", help="comma separated, e.g. m,[,[")
    ap.add_argument("--shot", default="", help="write a picture here")
    ap.add_argument("--frames", action="store_true", help="ask the game to time itself")
    ap.add_argument("--profile", action="store_true", help="sample the stack as well")
    ap.add_argument(
        "--names",
        default="",
        help="the .wasm being served, to turn wasm-function[N] into function names",
    )
    ap.add_argument("--quiet", action="store_true", help="only the summary")
    args = ap.parse_args()

    width, height = (int(n) for n in args.size.lower().split("x"))
    url = args.url
    if args.frames:
        url += ("&" if "?" in url else "?") + "frames=1"

    tab = Tab(url, (width, height))
    try:
        waited = tab.start()
        print(f"the card was up for {waited:.1f} s")
        if args.keys:
            tab.press([k for k in args.keys.split(",") if k])
        if args.profile:
            hottest, _ = profile(tab, args.seconds, wasm_names(args.names) if args.names else None)
        else:
            tab.pump(args.seconds)
            hottest = None
        if args.shot:
            tab.shot(args.shot)
            print(f"picture: {args.shot}")
        said = summarise(tab.console)
        if not args.quiet:
            for line in tab.console:
                if "frame:" in line or "slow frames" in line:
                    print(" ", line.strip())
        if said["reports"]:
            print(
                f"{args.size}: {said['fps']:.0f} fps mean, worst frame {said['worst']:.0f} ms"
                f" — sea {said['sea']:.2f} ms, foam {said['foam']:.2f} ms,"
                f" cloud {said['cloud']:.2f} ms"
            )
        if hottest:
            print("where the samples landed:")
            for name, share in hottest:
                print(f"  {share * 100:5.1f}%  {name}")
    finally:
        tab.close()


if __name__ == "__main__":
    main()
