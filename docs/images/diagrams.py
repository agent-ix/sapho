#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Agent-IX
"""Render Sapho's documentation diagrams as transparent light and dark SVGs.

Every label is drawn as IBM Plex glyph outlines, so GitHub and other viewers
show the same lettering without loading fonts. Markdown embeds each pair with
<picture> and a prefers-color-scheme source. Requires fontTools:

    python3 -m pip install fonttools
    python3 docs/images/diagrams.py

The Plex fonts (SIL Open Font License) download once into docs/images/.fonts/.
Animated figures play with CSS (no script, so they run inside GitHub's <img>)
and also get a NAME-still pair, their resting state, for "Show the whole
diagram" toggles. Readers who prefer reduced motion see that resting state.
Model answers in the figures are read from the recordings in examples/recordings/.
"""

import json
import re
import urllib.request
from pathlib import Path

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.ttLib import TTFont

OUT = Path(__file__).resolve().parent
FONT_DIR = OUT / ".fonts"
PLEX = "https://cdn.jsdelivr.net/npm/@ibm"
FONT_URLS = {
    "mono": f"{PLEX}/plex-mono@2.5.0/fonts/complete/woff/IBMPlexMono-Regular.woff",
    "title": f"{PLEX}/plex-sans-condensed@2.0.0/fonts/complete/woff/IBMPlexSansCondensed-SemiBold.woff",
}

# One meaning per colour in every figure.
THEMES = {
    "light": {
        "ink": "#161a24",
        "muted": "#586072",
        "model": "#0b7a78",
        "logic": "#4a3db0",
        "code": "#9a6208",
        "evidence": "#a3123f",
        "faint": "#c3c8d2",
        "tint": 0.10,
    },
    "dark": {
        "ink": "#e3e6ee",
        "muted": "#9aa1b3",
        "model": "#3ec5c1",
        "logic": "#a59bf7",
        "code": "#e5a94b",
        "evidence": "#f06a93",
        "faint": "#3d4453",
        "tint": 0.14,
    },
}

TITLE = 14
LABEL = 13
SMALL = 12
MARGIN = 6


class Font:
    """Glyph advances and outlines for one font face."""

    def __init__(self, key, path):
        self.key = key
        tt = TTFont(path)
        self.upem = tt["head"].unitsPerEm
        self.cmap = tt.getBestCmap()
        self.glyphs = tt.getGlyphSet()
        self.hmtx = tt["hmtx"]
        self._outlines = {}

    def glyph(self, ch):
        name = self.cmap.get(ord(ch))
        if name is None:
            raise ValueError(f"{self.key} has no glyph for {ch!r}")
        return name

    def advance(self, ch):
        return self.hmtx[self.glyph(ch)][0]

    def width(self, text, size):
        return sum(self.advance(ch) for ch in text) * size / self.upem

    def outline(self, name):
        if name not in self._outlines:
            pen = SVGPathPen(self.glyphs, ntos=lambda v: str(round(v)))
            self.glyphs[name].draw(pen)
            self._outlines[name] = pen.getCommands()
        return self._outlines[name]


FONTS = {}


def load_fonts():
    FONT_DIR.mkdir(exist_ok=True)
    for key, url in FONT_URLS.items():
        path = FONT_DIR / url.rsplit("/", 1)[1]
        if not path.exists():
            with urllib.request.urlopen(url, timeout=60) as response:
                path.write_bytes(response.read())
        FONTS[key] = Font(key, path)


def num(v):
    return f"{v:.1f}".rstrip("0").rstrip(".")


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;")


FADE = 0.2  # seconds an animated element takes to light up or fade


class Figure:
    """A fixed-size drawing whose colours resolve per theme at render time.

    With a `cycle` (seconds), elements can carry animation windows: "lit"
    elements are dimmed outside their windows, "appear" elements are hidden
    outside theirs, and tokens travel along an edge. Without animation every
    element shows its static state, which is also what readers who prefer
    reduced motion see.
    """

    def __init__(self, name, width, height, label, cycle=None, still_label=None):
        self.name = name
        self.width = width
        self.height = height
        self.label = label
        self.still_label = still_label or label
        self.cycle = cycle
        self.items = []
        self.anim = {}

    def _add(self, item, anim=None):
        self.items.append(item)
        if anim:
            if self.cycle is None:
                raise ValueError(f"{self.name}: animation needs a cycle")
            self.anim[len(self.items) - 1] = anim

    # ---- primitives -------------------------------------------------------
    def text(self, x, y, s, role="ink", font="mono", size=LABEL, anchor="start", anim=None):
        w = FONTS[font].width(s, size)
        x0 = {"start": x, "middle": x - w / 2, "end": x - w}[anchor]
        if x0 < -MARGIN or x0 + w > self.width + MARGIN or y - size < -MARGIN or y > self.height + MARGIN:
            raise ValueError(f"{self.name}: text {s!r} leaves the canvas")
        self._add(("text", x0, y, s, role, font, size), anim)
        return w

    def rect(self, x, y, w, h, role, dashed=False, fill=True, stroke=True, weight=None, opacity=None, rx=4, anim=None):
        self._add(("rect", x, y, w, h, role, dashed, fill, stroke, weight, opacity, rx), anim)

    def line(self, points, role="ink", weight=1.5, dashed=False, arrow=True, anim=None):
        self._add(("line", points, role, weight, dashed, arrow), anim)

    def dot(self, cx, cy, r, role, filled=True):
        self._add(("dot", cx, cy, r, role, filled))

    def bar(self, x, y, w, h, role):
        """A horizontal bar anchored at x with a rounded data end."""
        if w <= 0:
            return
        self._add(("bar", x, y, w, h, role))

    def token(self, points, role, start, end):
        """A dot that travels along `points` between `start` and `end` seconds."""
        self._add(("token", points, role), ("token", points, start, end))

    # ---- composites ------------------------------------------------------
    def box(self, x, y, w, h, role, title, *subs, dashed=False, sub_role="muted", lit=None):
        """A node: tinted box, title in the role colour, muted sub-lines.

        A title or sub-line may be a list of (text, windows, static) variants
        that appear only during their windows; `lit` dims the rest outside
        its windows.
        """
        glow = ("lit", lit) if lit else None
        self.rect(x, y, w, h, role, dashed=dashed, fill=role != "ink", anim=glow)
        lines = [(title, "title", TITLE, role)] + [(s, "mono", SMALL, sub_role) for s in subs]
        heights = [19 if f == "title" else 17 for _, f, _, _ in lines]
        if sum(heights) + 8 > h:
            raise ValueError(f"{self.name}: box {lines[0][0]!r} is too short")
        top = y + (h - sum(heights)) / 2
        for (s, f, size, r), lh in zip(lines, heights):
            variants = s if isinstance(s, list) else [(s, None, True)]
            for text, windows, static in variants:
                need = FONTS[f].width(text, size) + 16
                if need > w:
                    raise ValueError(f"{self.name}: {text!r} needs {need:.0f}px, box is {w}px")
                anim = ("appear", windows, static) if windows else glow
                self.text(x + w / 2, top + lh * 0.74, text, r, f, size, "middle", anim)
            top += lh

    def note(self, x, y, s, role, sub=None, anchor="start", anim=None):
        self.text(x, y, s, role, "mono", LABEL, anchor, anim)
        if sub:
            self.text(x, y + 16, sub, "muted", "mono", SMALL, anchor, anim)

    def legend(self, x, y, entries):
        for role, s, kind in entries:
            if kind == "box":
                self.rect(x, y - 10, 11, 11, role, weight=1.2, rx=2)
            elif kind == "dash":
                self.line([(x, y - 4), (x + 22, y - 4)], role, 1.5, dashed=True, arrow=False)
                x += 11
            else:
                self.rect(x, y - 10, 11, 11, role, stroke=False, opacity=1, rx=2)
            w = self.text(x + 17, y, s, "muted", "mono", SMALL)
            x += 17 + w + 24

    # ---- animation -------------------------------------------------------
    def _pct(self, t):
        return f"{100 * min(max(t, 0), self.cycle) / self.cycle:.3f}".rstrip("0").rstrip(".")

    def _keyframes(self, idx, spec):
        if spec[0] == "token":
            _, points, start, end = spec
            lengths = [abs(bx - ax) + abs(by - ay) for (ax, ay), (bx, by) in zip(points, points[1:])]
            total = sum(lengths)
            frames = [(0, points[0], 0), (start - 0.01, points[0], 0), (start, points[0], 1)]
            done = 0
            for length, point in zip(lengths, points[1:]):
                done += length
                frames.append((start + (end - start) * done / total, point, 1))
            frames += [(end + 0.15, points[-1], 0), (self.cycle, points[-1], 0)]
            body = " ".join(
                f"{self._pct(t)}%{{transform:translate({num(px)}px,{num(py)}px);opacity:{o}}}"
                for t, (px, py), o in frames
            )
        else:
            mode, windows = spec[0], spec[1]
            off = 0.22 if mode == "lit" else 0
            frames = [(0, off)]
            for start, end in windows:
                frames += [(start - FADE, off), (start, 1), (end, 1), (end + FADE, off)]
            frames.append((self.cycle, off))
            body = " ".join(f"{self._pct(t)}%{{opacity:{o}}}" for t, o in frames)
        return f"@keyframes k{idx}{{{body}}}.a{idx}{{animation:k{idx} {num(self.cycle)}s linear infinite}}"

    def _cls(self, idx):
        spec = self.anim.get(idx)
        if spec is None:
            return ""
        hidden = spec[0] == "token" or (spec[0] == "appear" and not spec[2])
        return f' class="a{idx}"' + (' opacity="0"' if hidden else "")

    # ---- rendering -------------------------------------------------------
    def render(self, theme, still=False):
        """SVG text; `still` drops the animation, leaving its resting state."""
        pal = THEMES[theme]
        used = {}
        body = []
        markers = set()
        for idx, item in enumerate(self.items):
            kind = item[0]
            cls = self._cls(idx)
            if kind == "rect":
                _, x, y, w, h, role, dashed, fill, stroke, weight, opacity, rx = item
                c = pal[role]
                attrs = [f'x="{num(x)}" y="{num(y)}" width="{num(w)}" height="{num(h)}" rx="{rx}"']
                if stroke:
                    attrs.append(f'stroke="{c}" stroke-width="{weight or (1.2 if role == "ink" else 1.5)}"')
                if fill:
                    attrs.append(f'fill="{c}" fill-opacity="{opacity if opacity is not None else pal["tint"]}"')
                else:
                    attrs.append('fill="none"')
                if dashed:
                    attrs.append('stroke-dasharray="5 4"')
                body.append(f"<rect{cls} {' '.join(attrs)}/>")
            elif kind == "line":
                _, points, role, weight, dashed, arrow = item
                d = "M" + " L".join(f"{num(px)},{num(py)}" for px, py in points)
                attrs = [f'd="{d}" fill="none" stroke="{pal[role]}" stroke-width="{weight}"']
                if dashed:
                    attrs.append('stroke-dasharray="5 4"')
                if arrow:
                    markers.add(role)
                    attrs.append(f'marker-end="url(#ah-{role})"')
                body.append(f"<path{cls} {' '.join(attrs)}/>")
            elif kind == "token":
                _, points, role = item
                body.append(f'<circle{cls} r="5" fill="{pal[role]}" stroke="{pal[role]}" stroke-opacity="0.35" stroke-width="5"/>')
            elif kind == "dot":
                _, cx, cy, r, role, filled = item
                c = pal[role]
                fill = f'fill="{c}"' if filled else 'fill="none"'
                body.append(f'<circle cx="{num(cx)}" cy="{num(cy)}" r="{r}" {fill} stroke="{c}" stroke-width="1.5"/>')
            elif kind == "bar":
                _, x, y, w, h, role = item
                r = min(4, h / 2, w)
                d = (
                    f"M{num(x)},{num(y)} H{num(x + w - r)} A{num(r)},{num(r)} 0 0 1 {num(x + w)},{num(y + r)} "
                    f"V{num(y + h - r)} A{num(r)},{num(r)} 0 0 1 {num(x + w - r)},{num(y + h)} H{num(x)} Z"
                )
                body.append(f'<path d="{d}" fill="{pal[role]}"/>')
            else:
                _, x, y, s, role, font, size = item
                f = FONTS[font]
                k = size / f.upem
                uses = []
                adv = 0
                for ch in s:
                    name = f.glyph(ch)
                    if f.outline(name):
                        gid = re.sub(r"[^A-Za-z0-9]", "_", f"{font}-{name}")
                        used[gid] = f.outline(name)
                        uses.append(f'<use href="#{gid}" x="{adv}"/>')
                    adv += f.advance(ch)
                body.append(
                    f'<g{cls} fill="{pal[role]}" transform="translate({num(x)},{num(y)}) '
                    f'scale({k:.5f},{-k:.5f})">{"".join(uses)}</g>'
                )
        defs = [f'<path id="{gid}" d="{d}"/>' for gid, d in sorted(used.items())]
        for role in sorted(markers):
            defs.append(
                f'<marker id="ah-{role}" viewBox="0 0 10 10" refX="9" refY="5" '
                f'markerWidth="7" markerHeight="7" orient="auto-start-reverse">'
                f'<path d="M0,0 L10,5 L0,10 z" fill="{pal[role]}"/></marker>'
            )
        style = ""
        if self.anim and not still:
            rules = "".join(self._keyframes(idx, spec) for idx, spec in sorted(self.anim.items()))
            style = (
                f"<style>{rules}"
                "@media (prefers-reduced-motion: reduce){*{animation:none!important}}</style>\n"
            )
        label = esc(self.still_label if still else self.label)
        w, h = self.width + 2 * MARGIN, self.height + 2 * MARGIN
        return (
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" '
            f'viewBox="{-MARGIN} {-MARGIN} {w} {h}" role="img" aria-label="{label}">\n'
            f"<title>{label}</title>\n{style}<defs>{''.join(defs)}</defs>\n" + "\n".join(body) + "\n</svg>\n"
        )

    def save(self):
        for theme in THEMES:
            (OUT / f"{self.name}-{theme}.svg").write_text(self.render(theme), encoding="utf-8")
            if self.anim:
                (OUT / f"{self.name}-still-{theme}.svg").write_text(self.render(theme, still=True), encoding="utf-8")


ROLES = [
    ("code", "your code", "box"),
    ("model", "model answer", "box"),
    ("logic", "your rule", "box"),
    ("ink", "input / decision", "box"),
]


# ---- recorded answers ------------------------------------------------------------

EXAMPLES = OUT.parents[1] / "examples"
# Pattern labels the requirement check counts as "has a condition".
CONDITIONAL = ("event", "state", "unwanted")
# `sapho tune` and held-out `sapho measure` results; scripts/check_docs.py pins them.
TUNED = (("0.7 / 0.8 rule", 1.0), ("0.9 / 0.9 rule", 0.67))
HELD_OUT_AGREEMENT = 1.0


def example(path):
    return json.loads((EXAMPLES / path).read_text(encoding="utf-8"))


def exchange(recording, backend, question, **state):
    """The recorded request and response that asked `question` of `backend` in this state."""
    for e in example(f"recordings/{recording}")["exchanges"]:
        request = e["request"]
        values = {k: v["value"] for k, v in request["state"]["value"].items()}
        if (
            request["backend"] == backend
            and any(q["id"] == question for q in request["questions"])
            and all(values.get(k) == v for k, v in state.items())
        ):
            return request, e["response"]
    raise LookupError(f"{recording}: no {backend} exchange asks {question} for {state}")


def question(request, qid):
    return next(q["question"] for q in request["questions"] if q["id"] == qid)


def code_review():
    """Jev's answers for examples/data/code-review-input.json."""
    case = example("data/code-review-input.json")
    request, response = exchange("code-review.json", "judge", "contract_risk", change=case["change"])
    answers = response["answers"]
    return {
        "request": request,
        "contract": answers["contract_risk"]["probability"],
        "gap": answers["test_gap"]["probability"],
        "api": case["public_api_changed"],
    }


STATEMENTS = (
    ("brake-lamp", "brake lamp"),
    ("cabin-light", "cabin light"),
    ("pump", "pump pressure"),
    ("report", "report promptly"),
    ("fast", "respond quickly"),
)


def requirement(name):
    """Jev's layer-1, layer-2 and expert answers for one requirement statement."""
    statement = example(f"data/requirements/{name}.json")["statement"]
    recording = "requirement-check.json"
    request, layer1 = exchange(recording, "judge", "kind", statement=statement)
    _, layer2 = exchange(recording, "judge", "complete", statement=statement)
    first = layer2["answers"]["complete"]["probability"]
    try:
        expert = exchange(recording, "expert", "complete", statement=statement)[1]["answers"]["complete"]["probability"]
    except LookupError:
        expert = None
    if (expert is not None) != (0.2 <= first < 0.8):
        raise ValueError(f"{name}: the recording disagrees with the graph's unsure band")
    kind = layer1["answers"]["kind"]
    return {
        "request": request,
        "kind": kind,
        "testable": layer1["answers"]["testable"],
        "conditional": sum(kind["probabilities"][label] for label in CONDITIONAL),
        "first": first,
        "expert": expert,
    }


def tf(value):
    return "true" if value else "false"


def wrap(text, width, size=SMALL):
    lines, line = [], ""
    for word in text.split():
        trial = f"{line} {word}".strip()
        if line and FONTS["mono"].width(trial, size) > width:
            lines.append(line)
            line = word
        else:
            line = trial
    return lines + [line]


# ---- figures -----------------------------------------------------------------


def how_it_works():
    """README hero: the code-review decision for one recorded change."""
    v = code_review()
    risky, untested = v["contract"] >= 0.7, v["gap"] >= 0.8
    either = risky or untested
    decision = v["api"] and either
    f = Figure(
        "how-it-works",
        980,
        356,
        f"A code change and two yes/no questions go to Jev in one request. Jev answers "
        f"{v['contract']:.2f} for contract risk and {v['gap']:.2f} for a test gap. "
        f"{v['contract']:.2f} {'clears' if risky else 'misses'} its 0.7 threshold and "
        f"{v['gap']:.2f} {'clears' if untested else 'misses'} its 0.8 threshold, so OR gives "
        f"{tf(either)}. AND with the public_api_changed fact from your code gives "
        f"needs_review = {tf(decision)}.",
    )
    cy = 170
    f.box(156, 4, 128, 64, "model", "questions", "contract_risk?", "test_gap?")
    f.box(0, cy - 28, 100, 56, "ink", "change", "the diff")
    f.box(156, cy - 28, 128, 56, "model", "ask Jev", "1 request")
    f.box(324, cy - 72, 136, 50, "model", "contract_risk", f"P(yes) = {v['contract']:.2f}")
    f.box(324, cy + 22, 136, 50, "model", "test_gap", f"P(yes) = {v['gap']:.2f}")
    f.box(500, cy - 72, 96, 50, "logic", "≥ 0.7", tf(risky))
    f.box(500, cy + 22, 96, 50, "logic", "≥ 0.8", tf(untested))
    f.box(636, cy - 25, 64, 50, "logic", "or", tf(either))
    f.box(500, 270, 200, 50, "code", "public_api_changed", f"your code · {tf(v['api'])}")
    f.box(740, cy - 25, 64, 50, "logic", "and", tf(decision))
    f.box(844, cy - 28, 136, 56, "ink", "needs_review", f"{tf(decision)} + trace")

    f.line([(100, cy), (153, cy)], "ink")
    f.note(108, cy - 9, "state", "muted")
    f.line([(220, 68), (220, cy - 31)], "model")
    f.line([(284, cy), (304, cy), (304, cy - 47), (321, cy - 47)], "model")
    f.line([(304, cy), (304, cy + 47), (321, cy + 47)], "model")
    f.note(312, cy + 5, "answers", "model")
    f.line([(460, cy - 47), (497, cy - 47)], "model")
    f.line([(460, cy + 47), (497, cy + 47)], "model")
    f.line([(596, cy - 47), (616, cy - 47), (616, cy - 10), (633, cy - 10)], "logic")
    f.line([(596, cy + 47), (616, cy + 47), (616, cy + 10), (633, cy + 10)], "logic")
    f.line([(700, cy), (737, cy)], "logic")
    f.line([(700, 295), (772, 295), (772, cy + 28)], "code")
    f.line([(804, cy), (841, cy)], "logic", weight=2.4)
    f.legend(0, 352, ROLES)
    f.save()


def model_answers():
    """What a model answer is: probability per label, and what `probability` counts."""
    review, fast = code_review(), requirement("fast")
    p = review["contract"]
    kind, testable = fast["kind"], fast["testable"]
    options = [o["label"] for o in question(fast["request"], "kind")["options"]]
    levels = [str(i) for i, _ in enumerate(question(fast["request"], "testable")["levels"])]
    top = len(levels) - 1
    panels = [
        (
            "yes / no",
            question(review["request"], "contract_risk")["instructions"],
            [("yes", p, True), ("no", 1 - p, False)],
            None,
            f"labels [true]  →  {p:.2f}",
        ),
        (
            "choice",
            question(fast["request"], "kind")["instructions"],
            [(o, kind["probabilities"][o], o in CONDITIONAL) for o in options],
            f"selected {kind['selected']} · confidence {kind['confidence']:.2f}",
            f"labels [{', '.join(CONDITIONAL)}]  →  {fast['conditional']:.2f}",
        ),
        (
            "score",
            question(fast["request"], "testable")["instructions"],
            [(f"level {lv}", testable["probabilities"][lv], lv == str(top)) for lv in levels],
            f"expected {testable['expected']:.2f} of {top} · confidence {testable['confidence']:.2f}",
            f"labels [{top}]  →  {testable['probabilities'][str(top)]:.2f}",
        ),
    ]

    def spoken(rows):
        return ", ".join(f"{label} {value:.2f}" for label, value, _ in rows)

    f = Figure(
        "model-answers",
        980,
        306,
        f"Three answers from Jev. Yes/no: {spoken(panels[0][2])}; counting label true gives "
        f"{p:.2f}. Choice: {spoken(panels[1][2])}, {panels[1][3]}; counting "
        f"{', '.join(CONDITIONAL)} gives {fast['conditional']:.2f}. Score: {spoken(panels[2][2])}, "
        f"{panels[2][3]}; counting level {top} gives {testable['probabilities'][str(top)]:.2f}.",
    )
    px = 0
    for title, text, rows, extra, projection in panels:
        f.text(px, 16, title, "ink", "title", TITLE)
        for i, line in enumerate(wrap(text, 300)):
            f.text(px, 36 + 16 * i, line, "muted", "mono", SMALL)
        y = 76
        for label, value, counted in rows:
            f.text(px, y + 10, label, "ink", "mono", SMALL)
            f.rect(px + 96, y, 160, 12, "faint", stroke=False, opacity=0.35, rx=2)
            f.bar(px + 96, y, 160 * value, 12, "model" if counted else "muted")
            f.text(px + 264, y + 10, f"{value:.2f}", "ink", "mono", SMALL)
            y += 24
        if extra:
            f.text(px, 190, extra, "muted", "mono", SMALL)
        f.line([(px, 214), (px + 300, 214)], "faint", 1, arrow=False)
        f.text(px, 238, "probability", "model", "title", TITLE)
        f.text(px, 258, projection, "model", "mono", SMALL)
        px += 340
    f.legend(0, 302, [("model", "counted by probability", "solid"), ("muted", "not counted", "solid")])
    f.save()


def combine_strengths():
    """Same three strengths, three policies, one threshold."""
    f = Figure(
        "combine-strengths",
        900,
        236,
        "Strengths A 0.8, B 0.4 and C 0.6. Minimum gives 0.40 and maximum 0.80; a weighted "
        "mean with weights 2, 1 and 1 gives 0.65. With review when the result is below 0.7, "
        "minimum and mean ask for review and maximum does not.",
    )
    x0, span = 236, 480

    def at(v):
        return x0 + span * v

    values = (("A", 0.8), ("B", 0.4), ("C", 0.6))
    weights = (2, 1, 1)
    f.text(0, 34, "strengths", "ink", "title", TITLE)
    f.line([(at(0), 30), (at(1), 30)], "faint", 1, arrow=False)
    for name, v in values:
        f.dot(at(v), 30, 6, "model")
        f.text(at(v), 16, f"{name} {v:.1f}", "ink", "mono", SMALL, "middle")
    xs = [v for _, v in values]
    rows = [
        ("min", "weakest part decides", min(xs)),
        ("max", "strongest part decides", max(xs)),
        ("weighted_mean", "weights 2 : 1 : 1", sum(w * x for w, x in zip(weights, xs)) / sum(weights)),
    ]
    y = 64
    for name, why, v in rows:
        f.text(0, y + 12, name, "logic", "title", TITLE)
        f.text(0, y + 29, why, "muted", "mono", SMALL)
        f.rect(at(0), y + 2, span, 16, "faint", stroke=False, opacity=0.35, rx=2)
        f.bar(at(0), y + 2, span * v, 16, "logic")
        f.text(at(1) + 24, y + 15, f"{v:.2f}", "ink", "mono", SMALL)
        verdict = "review" if v < 0.7 else "no review"
        f.text(at(1) + 80, y + 15, verdict, "evidence" if v < 0.7 else "muted", "title", TITLE)
        y += 46
    f.line([(at(0.7), 48), (at(0.7), y - 6)], "evidence", 1.5, dashed=True, arrow=False)
    f.text(at(0.7) + 6, y + 10, "review if < 0.7", "evidence", "mono", SMALL)
    for t in (0, 0.5, 1):
        f.text(at(t), y + 10, f"{t:g}", "muted", "mono", SMALL, "middle")
    f.save()


def layers():
    """An earlier answer becomes context for the next question."""
    v = requirement("report")
    event = v["kind"]["probabilities"]["event"]
    f = Figure(
        "layers",
        980,
        250,
        f"Layer 1 asks Jev which pattern the requirement follows. Its answer, has_condition "
        f"{v['conditional']:.2f}, is placed in the context for layer 2 next to the same "
        f"statement. Layer 2 asks whether the condition and response are stated and gets "
        f"{v['first']:.2f}, which goes on to the expert check.",
    )
    top, low = 62, 206
    f.text(0, 14, "LAYER 1", "muted", "mono", SMALL)
    f.text(0, 152, "LAYER 2", "muted", "mono", SMALL)
    f.box(0, top - 28, 150, 56, "ink", "statement", "“…it promptly.”")
    f.box(196, top - 28, 150, 56, "model", "ask Jev", "kind? · testable?")
    f.box(392, top - 31, 210, 62, "model", "kind", f"event {event:.2f}", f"has_condition = {v['conditional']:.2f}")
    f.box(196, low - 34, 210, 68, "logic", "context for layer 2", "statement", f"has_condition: {v['conditional']:.2f}")
    f.box(452, low - 28, 150, 56, "model", "ask Jev", "complete?")
    f.box(648, low - 28, 150, 56, "model", "first answer", f"P(yes) = {v['first']:.2f}")

    f.line([(150, top), (193, top)], "ink")
    f.line([(346, top), (389, top)], "model")
    f.line([(497, top + 31), (497, 130), (301, 130), (301, low - 37)], "model", weight=2.4)
    f.note(510, 116, "the layer-1 answer", "model", "becomes context")
    f.line([(75, top + 28), (75, low), (193, low)], "ink")
    f.note(84, low - 8, "same text", "muted")
    f.line([(406, low), (449, low)], "logic")
    f.line([(602, low), (645, low)], "model")
    f.line([(798, low), (880, low)], "model", weight=2.4)
    f.note(806, low - 9, "to the", "muted")
    f.text(806, low + 22, "expert check", "muted", "mono", SMALL)
    f.save()


def guard():
    """Ask the expert only when the first answer is unsure."""
    rows = [(label, requirement(name)) for name, label in STATEMENTS]
    asked = [r for _, r in rows if r["expert"] is not None]
    firsts = ", ".join(f"{r['first']:.2f}" for _, r in rows)
    unsure = ", ".join(f"{r['first']:.2f}" for r in asked)
    experts = ", ".join(f"{r['expert']:.2f}" for r in asked)
    f = Figure(
        "guard",
        980,
        420,
        f"First answers for {len(rows)} requirements: {firsts}. Only {unsure} falls in the "
        f"unsure band from 0.2 to 0.8, so only that statement goes to the expert, which "
        f"answers {experts}. The others keep their first answer. Coalesce picks the expert's "
        "answer when one exists, and the result is compared with 0.8.",
    )
    x0, span = 170, 640

    def at(v):
        return x0 + span * v

    f.text(0, 14, "first answer · P(complete)", "ink", "title", TITLE)
    f.rect(at(0.2), 28, span * 0.6, 24 * len(rows) + 8, "evidence", stroke=False, opacity=0.08, rx=2)
    f.text(at(0.5), 14, "unsure: 0.2 ≤ P < 0.8 → ask the expert", "evidence", "mono", SMALL, "middle")
    y = 44
    for label, r in rows:
        p, expert = r["first"], r["expert"]
        f.text(0, y + 4, label, "ink", "mono", SMALL)
        f.line([(at(0), y), (at(1), y)], "faint", 1, arrow=False)
        f.dot(at(p), y, 5.5, "model")
        if expert is None:
            f.text(at(p) + (12 if p < 0.9 else -12), y + 4, f"{p:.2f}", "ink", "mono", SMALL, "start" if p < 0.9 else "end")
        else:
            f.line([(at(p) - 8, y), (at(expert) + 9, y)], "evidence", 1.5, dashed=True)
            f.dot(at(expert), y, 5.5, "evidence", filled=False)
            f.text(at(p) + 12, y + 4, f"{p:.2f}", "ink", "mono", SMALL)
            f.text(at(expert), y - 10, f"expert {expert:.2f}", "evidence", "mono", SMALL, "middle")
        y += 24
    for t in (0, 0.2, 0.5, 0.8, 1):
        f.text(at(t), y + 8, f"{t:g}", "muted", "mono", SMALL, "middle")

    fy = 330
    f.box(0, fy - 28, 140, 56, "model", "first answer", "layer 2")
    f.box(184, fy - 28, 150, 56, "logic", "unsure?", "0.2 ≤ P < 0.8")
    f.box(420, fy - 80, 150, 56, "model", "ask the expert", "guarded", dashed=True)
    f.box(620, fy - 28, 150, 56, "logic", "coalesce", "expert, else first")
    f.box(830, fy - 28, 150, 56, "ink", "complete ≥ 0.8", "ok")
    f.line([(140, fy), (181, fy)], "model")
    f.line([(334, fy - 14), (370, fy - 14), (370, fy - 52), (417, fy - 52)], "logic", dashed=True)
    f.note(378, fy - 26, "true", "logic")
    f.line([(570, fy - 52), (595, fy - 52), (595, fy - 10), (617, fy - 10)], "model", dashed=True)
    if len(asked) == 1:
        f.note(578, fy - 62, f"{asked[0]['expert']:.2f}", "model")
    f.line([(70, fy + 28), (70, fy + 62), (695, fy + 62), (695, fy + 31)], "model")
    f.note(380, fy + 54, "the first answer, kept when the expert is skipped", "muted", anchor="middle")
    f.line([(770, fy), (827, fy)], "logic", weight=2.4)
    f.legend(0, 420 - 2, [("model", f"skipped for {len(rows) - len(asked)} of {len(rows)} statements", "dash")])
    f.save()


def collections():
    """Many items: map, filter and pairs keep each item's identity."""
    f = Figure(
        "collections",
        980,
        210,
        "n statements are mapped through a model subgraph, giving one answer per statement. "
        "filter keeps the k that pass, pairs forms every candidate with m components, a "
        "second map judges each candidate, and findings keep the IDs of the items they came from.",
    )
    cy = 70
    f.box(0, cy - 30, 130, 60, "ink", "statements", "n items")
    f.box(170, cy - 30, 150, 60, "model", "map", "ask each item")
    f.box(360, cy - 30, 120, 60, "logic", "filter", "mask: passed")
    f.box(520, cy - 30, 130, 60, "logic", "pairs", "k × m")
    f.box(690, cy - 30, 130, 60, "model", "map", "judge each pair")
    f.box(860, cy - 30, 120, 60, "ink", "findings", "item IDs kept")
    f.box(360, 134, 120, 50, "ink", "components", "m items")
    f.line([(130, cy), (167, cy)], "ink")
    f.line([(320, cy), (357, cy)], "model")
    f.note(325, cy - 9, "n", "model")
    f.line([(480, cy), (517, cy)], "logic")
    f.note(487, cy - 9, "k", "logic")
    f.line([(480, 159), (585, 159), (585, cy + 33)], "ink")
    f.line([(650, cy), (687, cy)], "logic")
    f.line([(820, cy), (857, cy)], "model", weight=2.4)
    f.text(0, 128, "each item carries its ID", "evidence", "mono", SMALL)
    f.text(0, 145, "and sources through every step", "evidence", "mono", SMALL)
    f.save()


def evidence_loop():
    """Record once, then measure and tune offline against labelled cases.

    The animation follows one round: record the cases, replay both candidate
    rules from the recording, check the winner on held-out cases and export
    the development cases. Without motion every step shows at once.
    """
    cases = example("data/code-review-dataset.json")["cases"]
    development = sum(c["split"] == "development" for c in cases)
    held_out = sum(c["split"] == "held_out" for c in cases)
    saved = len(example("recordings/code-review.json")["exchanges"])
    (best, best_score), (other, other_score) = TUNED
    go = {"start": 0.2, "graph": 0.4, "cases": 0.4, "answers": 1.5, "replay": 3.4, "pick": 5.2,
          "export": 6.3, "trainer": 7.2, "end": 9.8}
    f = Figure(
        "evidence-loop",
        980,
        300,
        f"A graph and a labelled dataset of {len(cases)} cases, {development} for development "
        f"and {held_out} held out. sapho record runs once per case against Jev and the "
        f"{saved} answers are saved in one recording. Tune and measure then run offline: "
        f"tuning on development cases scores the {best} {best_score:.2f} and the {other} "
        f"{other_score:.2f}. The chosen rule is measured once on held-out cases, and "
        "development cases can be exported for a trainer outside Sapho.",
        cycle=go["end"] + 0.5,
    )
    end = go["end"]

    def lit(t):
        return [(t, end)]

    def shown(text, t):
        return [(text, [(t, end)], True)]

    stream = [go["answers"] + 0.16 * i for i in range(saved)]
    f.box(0, 20, 160, 56, "logic", "graph", "code-review.yaml", lit=lit(go["start"]))
    f.box(0, 150, 160, 70, "evidence", "labelled cases", f"{development} development", f"{held_out} held out",
          lit=lit(go["start"]))
    f.box(212, 20, 160, 56, "model", "sapho record", "Jev · once per case", lit=lit(go["graph"] + 0.8))
    f.box(424, 20, 160, 56, "evidence", "recording", shown(f"{saved} saved answers", stream[-1] + 0.5),
          lit=lit(stream[0] + 0.5))
    f.box(660, 14, 190, 68, "logic", "sapho tune", shown(f"{best}  {best_score:.2f}", go["replay"] + 0.6),
          shown(f"{other}  {other_score:.2f}", go["replay"] + 1.2), lit=lit(go["replay"] + 0.6))
    f.box(660, 150, 190, 70, "logic", "sapho measure", "held out · once",
          shown(f"agreement {HELD_OUT_AGREEMENT:.2f}", go["pick"] + 0.6), lit=lit(go["pick"] + 0.5))
    f.box(424, 150, 160, 70, "evidence", "export-training", "development only", "JSONL",
          lit=lit(go["export"] + 0.8))
    f.box(424, 248, 160, 52, "ink", "your trainer", "outside Sapho", dashed=True, lit=lit(go["trainer"] + 0.3))

    edges = [  # points, role, weight, dashed, token start times, travel
        ([(160, 48), (209, 48)], "logic", 1.5, False, [go["graph"]], 0.4),
        ([(160, 185), (186, 185), (186, 60), (209, 60)], "evidence", 1.5, False, [go["cases"]], 0.8),
        ([(372, 48), (421, 48)], "model", 1.5, False, stream, 0.5),
        ([(584, 48), (657, 48)], "evidence", 2.4, False, [go["replay"], go["replay"] + 0.6], 0.6),
        ([(755, 82), (755, 147)], "logic", 1.5, False, [go["pick"]], 0.5),
        ([(160, 200), (421, 200)], "evidence", 1.5, False, [go["export"] + 0.2 * i for i in range(development)], 0.6),
        ([(504, 220), (504, 245)], "evidence", 1.5, True, [go["trainer"]], 0.3),
    ]
    for points, role, weight, dashed, starts, travel in edges:
        f.line(points, role, weight, dashed, anim=("lit", [(starts[0], end)]))
        for t in starts:
            f.token(points, role, t, t + travel)
    f.note(380, 40, "live", "model")
    f.note(592, 40, "offline", "evidence")
    f.note(764, 120, "pick the winner", "logic")
    f.save()


def multi_stage():
    """README: three model layers for one statement, the last only when unsure.

    The animation plays two recorded statements in a loop: one that layer 2 is
    unsure about, so the expert is asked, and one it is sure about, which skips
    the expert. Without motion it shows the first statement.
    """
    runs, t = [], 0.2
    for name, quote in (("report", "“…it promptly.”"), ("brake-lamp", "“…within 100 ms.”")):
        v = requirement(name)
        v["quote"] = quote
        v["unsure"] = 0.2 <= v["first"] < 0.8
        v["final"] = v["expert"] if v["unsure"] else v["first"]
        v["ok"] = v["final"] >= 0.8
        go = {"statement": t, "ask1": t + 0.3, "kind": t + 1.1, "ctx": t + 1.8, "same": t + 1.8,
              "ask2": t + 3.1, "first": t + 3.8, "unsure": t + 4.5}
        if v["unsure"]:
            go.update(expert=t + 5.2, answer=t + 6.5, ok=t + 7.2, end=t + 9.6)
        else:
            go.update(sure=t + 5.2, end=t + 7.8)
        v["go"] = go
        runs.append(v)
        t = go["end"] + 0.5
    v = runs[0]
    rest = [requirement(name) for name, _ in STATEMENTS]
    skipped = sum(r["expert"] is None for r in rest)
    still = (
        f"Three model layers for one requirement. Layer 1 asks Jev for its pattern: event "
        f"{v['kind']['probabilities']['event']:.2f}, so has_condition is {v['conditional']:.2f}. "
        f"Layer 2 sees that answer next to the statement and asks whether the condition and "
        f"response are stated: {v['first']:.2f}. That is in the unsure band from 0.2 to 0.8, so "
        f"layer 3 asks the expert, which answers {v['expert']:.2f}, below the 0.8 needed to pass. "
        f"{skipped} of {len(rest)} recorded statements never reach layer 3."
    )
    f = Figure(
        "multi-stage",
        980,
        392,
        f"{still} The animation then replays a statement layer 2 is sure about "
        f"({runs[1]['first']:.2f}), which skips the expert.",
        cycle=t,
        still_label=still,
    )
    r1, r2, r3 = 50, 170, 296
    edges = {  # key: (points, role, weight, dashed, travel seconds)
        "ask1": ([(150, r1), (193, r1)], "ink", 1.5, False, 0.4),
        "kind": ([(346, r1), (389, r1)], "model", 1.5, False, 0.4),
        "ctx": ([(497, r1 + 31), (497, 108), (301, 108), (301, r2 - 37)], "model", 2.4, False, 1.0),
        "same": ([(104, r1 + 28), (104, r2), (193, r2)], "ink", 1.5, False, 1.0),
        "ask2": ([(406, r2), (449, r2)], "logic", 1.5, False, 0.4),
        "first": ([(602, r2), (645, r2)], "model", 1.5, False, 0.4),
        "unsure": ([(798, r2), (835, r2)], "model", 1.5, False, 0.4),
        "expert": ([(909, r2 + 28), (909, 236), (527, 236), (527, r3 - 31)], "logic", 1.5, True, 1.0),
        "answer": ([(602, r3), (645, r3)], "model", 1.5, True, 0.4),
        "ok": ([(798, r3), (835, r3)], "model", 2.4, False, 0.4),
        "sure": ([(960, r2 + 28), (960, r3 - 31)], "logic", 1.5, False, 0.6),
    }
    # Which box each edge delivers to, so the box lights up on arrival.
    target = {"ask1": "ask1", "kind": "kind", "ctx": "ctx", "same": "ctx", "ask2": "ask2", "first": "first",
              "unsure": "unsure", "expert": "expert", "answer": "answer", "ok": "ok", "sure": "ok"}

    def arrive(r, box):
        if box == "statement":
            return r["go"]["statement"]
        times = [r["go"][e] + edges[e][4] for e, b in target.items() if b == box and e in r["go"]]
        return min(times) if times else None

    def lit(box):
        return [(arrive(r, box), r["go"]["end"]) for r in runs if arrive(r, box) is not None]

    def value(box, fmt):
        """One variant per run that reaches the box, shown from arrival to the run's end."""
        return [(fmt(r), [(arrive(r, box), r["go"]["end"])], i == 0)
                for i, r in enumerate(runs) if arrive(r, box) is not None]

    f.text(0, 12, "LAYER 1 · Jev", "muted", "mono", SMALL)
    f.text(0, 132, "LAYER 2 · Jev", "muted", "mono", SMALL)
    f.text(0, 258, "LAYER 3 · expert, only when unsure", "muted", "mono", SMALL)
    f.box(0, r1 - 28, 150, 56, "ink", "statement", value("statement", lambda r: r["quote"]), lit=lit("statement"))
    f.box(196, r1 - 28, 150, 56, "model", "ask Jev", "kind? · testable?", lit=lit("ask1"))
    f.box(392, r1 - 31, 210, 62, "model", "kind",
          value("kind", lambda r: f"event {r['kind']['probabilities']['event']:.2f}"),
          value("kind", lambda r: f"has_condition = {r['conditional']:.2f}"), lit=lit("kind"))
    f.box(196, r2 - 34, 210, 68, "logic", "context for layer 2", "statement",
          value("ctx", lambda r: f"has_condition: {r['conditional']:.2f}"), lit=lit("ctx"))
    f.box(452, r2 - 28, 150, 56, "model", "ask Jev", "complete?", lit=lit("ask2"))
    f.box(648, r2 - 28, 150, 56, "model", "first answer", value("first", lambda r: f"P(yes) = {r['first']:.2f}"),
          lit=lit("first"))
    f.box(838, r2 - 28, 142, 56, "logic", "unsure?", value("unsure", lambda r: f"0.2 ≤ {r['first']:.2f} < 0.8"),
          lit=lit("unsure"))
    f.box(452, r3 - 28, 150, 56, "model", "ask the expert", "guarded", dashed=True, lit=lit("expert"))
    f.box(648, r3 - 28, 150, 56, "model", "expert answer", value("answer", lambda r: f"P(yes) = {r['expert']:.2f}"),
          lit=lit("answer"))
    f.box(838, r3 - 28, 142, 56, "ink", value("ok", lambda r: f"ok = {tf(r['ok'])}"),
          value("ok", lambda r: f"{r['final']:.2f} {'≥' if r['ok'] else '<'} 0.8"), lit=lit("ok"))

    for key, (points, role, weight, dashed, travel) in edges.items():
        windows = [(r["go"][key], r["go"]["end"]) for r in runs if key in r["go"]]
        f.line(points, role, weight, dashed, anim=("lit", windows))
        for r in runs:
            if key in r["go"]:
                f.token(points, role, r["go"][key], r["go"][key] + travel)
    f.note(510, 104, "the layer-1 answer", "model", "becomes context")
    f.note(110, r2 - 8, "same text", "muted")

    def branch(taken):
        return ("appear", [(arrive(r, "unsure"), r["go"]["end"]) for r in runs if r["unsure"] == taken], True)

    f.note(535, 254, "true", "logic", anim=branch(True))
    f.note(952, 252, "false", "logic", anchor="end", anim=branch(False))
    f.legend(0, 388, ROLES[1:] + [("model", f"skipped for {skipped} of {len(rest)} statements", "dash")])
    f.save()


def main():
    load_fonts()
    how_it_works()
    multi_stage()
    model_answers()
    combine_strengths()
    layers()
    guard()
    collections()
    evidence_loop()


if __name__ == "__main__":
    main()
