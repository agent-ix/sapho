#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Agent-IX
"""Render original, transparent Sapho diagrams from exact labels and graph edges.

Requires Matplotlib. PNGs are embedded in Markdown; SVGs are scalable originals.
    MPLCONFIGDIR=/tmp/sapho-docs-mpl python3 docs/images/generate.py
For visual inspection on both page themes, add --preview-dir /tmp/sapho-preview.
"""

import argparse
from dataclasses import dataclass
from pathlib import Path

import matplotlib

import matplotlib.patheffects as effects
import matplotlib.pyplot as plt
from matplotlib.patches import (
    Circle,
    FancyArrowPatch,
    FancyBboxPatch,
    PathPatch,
    Polygon,
)
from matplotlib.path import Path as PlotPath

matplotlib.use("Agg")


OUT = Path(__file__).resolve().parent
INK = "#24363e"
PAPER = "#fff4d9"
BLUE = "#dce9fc"
MINT = "#d3efdf"
PEACH = "#ffe1cc"
PINK = "#f7dced"
HALO = "#fff4d9"
VALUES = (0.8, 0.4, 0.6)
WEIGHTS = (2.0, 1.0, 1.0)
MEAN = sum(w * x for w, x in zip(WEIGHTS, VALUES)) / sum(WEIGHTS)
PREVIEW_DIR = None

plt.rcParams.update(
    {
        "font.family": ["Lato", "DejaVu Sans"],
        "font.size": 12,
        "text.color": INK,
        "svg.fonttype": "none",
        "svg.hashsalt": "sapho-friendly-diagrams",
    }
)


@dataclass(frozen=True)
class Node:
    x: float
    y: float
    w: float
    h: float

    def anchor(self, side):
        return {
            "left": (self.x - 0.8, self.y + self.h / 2),
            "right": (self.x + self.w + 0.8, self.y + self.h / 2),
            "top": (self.x + self.w / 2, self.y + self.h + 0.8),
            "bottom": (self.x + self.w / 2, self.y - 0.8),
            "bottom_left": (self.x + self.w / 4, self.y - 0.8),
        }[side]


def canvas(height=6.0):
    fig, ax = plt.subplots(figsize=(11.5, height))
    fig.patch.set_alpha(0)
    ax.patch.set_alpha(0)
    ax.set(xlim=(0, 100), ylim=(0, 100))
    ax.axis("off")
    fig.subplots_adjust(left=0.015, right=0.985, bottom=0.015, top=0.985)
    return fig, ax


def sticker(ax, x, y, w, h, color):
    shadow = FancyBboxPatch(
        (x + 0.45, y - 0.65),
        w,
        h,
        boxstyle="round,pad=0.4,rounding_size=2.2",
        facecolor=INK,
        edgecolor="none",
        alpha=0.16,
        zorder=2,
    )
    ax.add_patch(shadow)
    # A few gently uneven Bezier edges give a doodled outline without the
    # thousands of resampled vertices emitted by Matplotlib's sketch filter.
    r = min(2.2, h / 3, w / 5)
    vertices = [
        (x + r, y),
        (x + w * 0.28, y - 0.18),
        (x + w * 0.72, y + 0.18),
        (x + w - r, y),
        (x + w, y),
        (x + w, y + r),
        (x + w + 0.14, y + h * 0.3),
        (x + w - 0.14, y + h * 0.7),
        (x + w, y + h - r),
        (x + w, y + h),
        (x + w - r, y + h),
        (x + w * 0.72, y + h + 0.2),
        (x + w * 0.28, y + h - 0.2),
        (x + r, y + h),
        (x, y + h),
        (x, y + h - r),
        (x - 0.14, y + h * 0.7),
        (x + 0.14, y + h * 0.3),
        (x, y + r),
        (x, y),
        (x + r, y),
        (x + r, y),
    ]
    codes = (
        [PlotPath.MOVETO]
        + [PlotPath.CURVE4] * 3
        + [PlotPath.CURVE3] * 2
        + [PlotPath.CURVE4] * 3
        + [PlotPath.CURVE3] * 2
        + [PlotPath.CURVE4] * 3
        + [PlotPath.CURVE3] * 2
        + [PlotPath.CURVE4] * 3
        + [PlotPath.CURVE3] * 2
        + [PlotPath.CLOSEPOLY]
    )
    patch = PathPatch(
        PlotPath(vertices, codes),
        facecolor=color,
        edgecolor=INK,
        linewidth=2.0,
        zorder=3,
    )
    patch.set_path_effects(
        [effects.Stroke(linewidth=4.2, foreground=HALO), effects.Normal()]
    )
    ax.add_patch(patch)


def pill(ax, x, y, w, h, text, color=PAPER, size=12, bold=False):
    sticker(ax, x, y, w, h, color)
    ax.text(
        x + w / 2,
        y + h / 2,
        text,
        ha="center",
        va="center",
        fontsize=size,
        fontweight="bold" if bold else "normal",
        zorder=6,
    )


def heading(ax, title, subtitle):
    pill(ax, 3, 86, 94, 10, title, size=21, bold=True)
    pill(ax, 8, 75.5, 84, 7, subtitle, color=MINT, size=11.8)


def icon(ax, x, y, kind):
    line = {"color": INK, "linewidth": 1.7, "zorder": 6, "solid_capstyle": "round"}
    if kind == "model":
        ax.add_patch(
            FancyBboxPatch(
                (x - 2.7, y - 1.8),
                5.4,
                3.6,
                boxstyle="round,pad=0.1,rounding_size=0.9",
                facecolor=PAPER,
                edgecolor=INK,
                linewidth=1.6,
                zorder=6,
            )
        )
        ax.plot([x, x], [y + 1.9, y + 3.0], **line)
        ax.add_patch(Circle((x, y + 3.0), 0.35, facecolor=INK, zorder=7))
        for dx in [-1.1, 1.1]:
            ax.add_patch(Circle((x + dx, y + 0.3), 0.27, facecolor=INK, zorder=7))
        ax.plot([x - 0.6, x, x + 0.6], [y - 0.65, y - 0.85, y - 0.65], **line)
    elif kind == "code":
        ax.text(
            x,
            y,
            "{ }",
            ha="center",
            va="center",
            fontfamily="DejaVu Sans Mono",
            fontsize=19,
            fontweight="bold",
            zorder=6,
        )
    elif kind == "document":
        ax.add_patch(
            Polygon(
                [
                    (x - 2, y - 2.2),
                    (x + 2, y - 2.2),
                    (x + 2, y + 1),
                    (x + 0.7, y + 2.2),
                    (x - 2, y + 2.2),
                ],
                closed=True,
                facecolor=PAPER,
                edgecolor=INK,
                linewidth=1.6,
                zorder=6,
            )
        )
        for dy in [-1, 0, 1]:
            ax.plot([x - 1.2, x + 0.7], [y + dy, y + dy], **line)
    elif kind == "logic":
        ax.plot([x - 2.5, x, x + 2.5], [y + 1.5, y, y + 1.5], **line)
        ax.plot([x, x], [y, y - 2.0], **line)
        for px, py in [(x - 2.5, y + 1.5), (x + 2.5, y + 1.5), (x, y - 2.0)]:
            ax.add_patch(
                Circle(
                    (px, py),
                    0.6,
                    facecolor=PAPER,
                    edgecolor=INK,
                    linewidth=1.4,
                    zorder=7,
                )
            )
    else:
        ax.add_patch(
            Circle((x, y), 2.2, facecolor=PAPER, edgecolor=INK, linewidth=1.6, zorder=6)
        )
        ax.plot([x - 1.1, x - 0.25, x + 1.3], [y, y - 0.9, y + 1.1], **line)


def card(ax, x, y, w, h, title, body, color=BLUE, kind="code", size=12.8):
    sticker(ax, x, y, w, h, color)
    icon(ax, x + w / 2, y + h * 0.84, kind)
    ax.text(
        x + w / 2,
        y + h * 0.61,
        title,
        ha="center",
        va="center",
        fontsize=size,
        fontweight="bold",
        zorder=6,
    )
    ax.text(
        x + w / 2,
        y + h * 0.25,
        body,
        ha="center",
        va="center",
        fontsize=11.2,
        linespacing=1.35,
        zorder=6,
    )
    return Node(x, y, w, h)


def connect(ax, start, end, src="right", dst="left", bend=0, dashed=False):
    arrow = FancyArrowPatch(
        start.anchor(src),
        end.anchor(dst),
        connectionstyle=f"arc3,rad={bend}",
        arrowstyle="-|>",
        mutation_scale=18,
        color=INK,
        linewidth=2.0,
        zorder=1,
        linestyle="--" if dashed else "-",
    )
    arrow.set_path_effects(
        [effects.Stroke(linewidth=4.5, foreground=HALO), effects.Normal()]
    )
    ax.add_patch(arrow)


def save(fig, name):
    metadata = {
        "Software": "Sapho documentation renderer",
        "Copyright": "Copyright (C) 2026 Agent-IX",
        "License": "SPDX-License-Identifier: AGPL-3.0-or-later",
    }
    fig.savefig(OUT / f"{name}.png", dpi=160, transparent=True, metadata=metadata)
    fig.savefig(
        OUT / f"{name}.svg",
        transparent=True,
        metadata={
            "Date": None,
            "Creator": metadata["Software"],
            "Rights": metadata["License"],
        },
    )
    svg = OUT / f"{name}.svg"
    svg.write_text(
        "\n".join(line.rstrip() for line in svg.read_text().splitlines()) + "\n"
    )
    if PREVIEW_DIR is not None:
        for theme, background in [("dark", "#0d1117"), ("light", "#ffffff")]:
            fig.patch.set_alpha(1)
            fig.savefig(
                PREVIEW_DIR / f"{name}-{theme}.png",
                dpi=110,
                facecolor=background,
                transparent=False,
            )
        fig.patch.set_alpha(0)
    plt.close(fig)


def evaluation_flow():
    fig, ax = canvas()
    heading(
        ax,
        "Layer your thinking",
        "Rust code + model judgments + logic, connected in one graph",
    )
    source = card(
        ax, 3, 42, 16, 26, "Your inputs", "Code units\nRequirements", PAPER, "document"
    )
    prepare = card(ax, 23, 42, 16, 26, "Prepare", "Extract items\nBuild state", BLUE)
    model1 = card(
        ax,
        43,
        42,
        16,
        26,
        "Model layer 1",
        "Classify\nFind candidates",
        MINT,
        "model",
        12.4,
    )
    next_state = card(
        ax,
        63,
        42,
        16,
        26,
        "Rust + logic",
        "Select evidence\nBuild next questions",
        PEACH,
        "logic",
        12.4,
    )
    model2 = card(
        ax,
        83,
        42,
        14,
        26,
        "Model layer 2",
        "Judge links\nUse prior evidence",
        MINT,
        "model",
        11.6,
    )
    facts = card(
        ax, 24, 8, 24, 24, "Independent facts", "Static checks / native rules", BLUE
    )
    combine = card(
        ax, 56, 8, 18, 24, "Combine", "Hard gates\nSoft strengths", PEACH, "logic"
    )
    output = card(
        ax, 81, 8, 16, 24, "Findings", "Decisions\n+ evidence", PINK, "result"
    )
    for a, b in [
        (source, prepare),
        (prepare, model1),
        (model1, next_state),
        (next_state, model2),
        (facts, combine),
        (combine, output),
    ]:
        connect(ax, a, b)
    connect(ax, source, facts, "bottom", "left", bend=0.25)
    connect(ax, model2, combine, "bottom", "top", bend=-0.15)
    save(fig, "evaluation-flow")


def logic_flow():
    fig, ax = canvas()
    heading(
        ax,
        "One graph. Distinct meanings.",
        "Turn probability evidence into a chosen policy, then a decision",
    )
    probability = card(
        ax,
        3,
        42,
        19,
        26,
        "Probability",
        "Project a model's\noutcome mass",
        MINT,
        "model",
    )
    degree = card(
        ax, 28, 42, 19, 26, "Degree", "Interpret on\na 0–1 scale", BLUE, "logic"
    )
    combined = card(
        ax,
        53,
        42,
        22,
        26,
        "Combined degree",
        "min / max\nweighted mean",
        PEACH,
        "logic",
    )
    decision = card(
        ax, 81, 42, 16, 26, "Boolean", "Compare with\na threshold", PINK, "result"
    )
    for a, b in [(probability, degree), (degree, combined), (combined, decision)]:
        connect(ax, a, b)
    others = card(
        ax,
        29,
        7,
        27,
        24,
        "Other strengths",
        "Rust facts mapped to degrees\nOther model evidence",
        BLUE,
    )
    connect(ax, others, combined, "right", "bottom", bend=0.15)
    pill(ax, 65, 12, 32, 12, "0.65 < 0.7 → true", color=PINK, size=14, bold=True)
    save(fig, "logic-flow")


def degree_combiners():
    fig, ax = canvas(6.7)
    heading(
        ax,
        "Same evidence. Different policies.",
        "Strengths 0.8, 0.4, 0.6 · weights 2 : 1 : 1 for the mean",
    )
    inputs = card(
        ax, 3, 27, 23, 32, "Evidence", "A = 0.8\nB = 0.4\nC = 0.6", BLUE, "document", 15
    )
    minimum = card(
        ax,
        36,
        47,
        25,
        22,
        "Minimum → 0.40",
        "Use the weakest part",
        PEACH,
        "logic",
        13.5,
    )
    maximum = card(
        ax,
        36,
        18,
        25,
        22,
        "Maximum → 0.80",
        "Use the strongest part",
        MINT,
        "logic",
        13.5,
    )
    mean = card(
        ax,
        71,
        47,
        26,
        22,
        f"Mean → {MEAN:.2f}",
        "Balance with weights 2:1:1",
        PINK,
        "logic",
        14,
    )
    complement = card(
        ax,
        71,
        18,
        26,
        22,
        f"Complement → {1 - MEAN:.2f}",
        f"1 − {MEAN:.2f}",
        PAPER,
        "logic",
        12.5,
    )
    connect(ax, inputs, minimum, bend=-0.18)
    connect(ax, inputs, maximum, bend=0.18)
    connect(ax, inputs, mean, "right", "bottom_left", bend=0.02)
    connect(ax, mean, complement, "bottom", "top")
    pill(
        ax,
        7,
        2,
        86,
        9,
        "These are heuristic strengths; a separate comparison chooses the decision.",
        size=11.4,
    )
    save(fig, "degree-combiners")


def multi_layer_requirements():
    fig, ax = canvas(6.7)
    heading(
        ax,
        "Requirements, one layer at a time",
        "Illustrative application graph · later questions use earlier evidence",
    )
    prepare = card(
        ax,
        3,
        45,
        20,
        24,
        "Prepare text",
        "Statement records\n+ source spans",
        BLUE,
        "code",
    )
    structure = card(
        ax, 28, 45, 20, 24, "1 · Structure", "Patterns\n+ conditions", MINT, "model"
    )
    roles = card(
        ax, 53, 45, 20, 24, "2 · Roles", "Actor / action\n/ target", MINT, "model"
    )
    candidates = card(
        ax,
        78,
        45,
        19,
        24,
        "Build candidates",
        "Rust + filter\n+ pairs or join",
        BLUE,
        "code",
        12,
    )
    relationships = card(
        ax,
        78,
        10,
        19,
        24,
        "3 · Relationships",
        "Judge selected\nrole and condition links",
        MINT,
        "model",
        11.5,
    )
    select = card(
        ax,
        53,
        10,
        20,
        24,
        "Need an expert?",
        "Compare support\n→ Boolean guard",
        PEACH,
        "logic",
        12,
    )
    expert = card(
        ax,
        28,
        10,
        20,
        24,
        "4 · Expert check",
        "Ask when guarded\n→ Optional evidence",
        MINT,
        "model",
        12,
    )
    findings = card(
        ax,
        3,
        10,
        20,
        24,
        "Assemble findings",
        "Rust + combinations\n+ source references",
        PINK,
        "result",
        11.8,
    )
    for a, b in [(prepare, structure), (structure, roles), (roles, candidates)]:
        connect(ax, a, b)
    connect(ax, candidates, relationships, "bottom", "top")
    for a, b in [(relationships, select), (select, expert), (expert, findings)]:
        connect(ax, a, b, "left", "right")
    connect(ax, relationships, findings, "bottom", "bottom", bend=-0.10, dashed=True)
    pill(ax, 38, 0.5, 24, 5.5, "Earlier evidence", size=10.5)
    save(fig, "multi-layer-requirements")


def code_review_combinations():
    fig, ax = canvas(6.7)
    heading(
        ax,
        "Hard facts meet soft evidence",
        "changed_public_api AND (contract_risk ≥ 0.7 OR test_gap ≥ 0.8)",
    )
    fact = card(
        ax, 64, 43, 14, 26, "API changed?", "Rust Boolean\n→ true", BLUE, "code", 11.6
    )
    risk = card(
        ax, 3, 9, 23, 25, "Contract risk", "Model → degree 0.72", MINT, "model", 13
    )
    risk_cut = card(
        ax, 33, 9, 24, 25, "Risk ≥ 0.7?", "0.72 ≥ 0.7 → true", PEACH, "logic", 13
    )
    tests = card(
        ax,
        33,
        43,
        24,
        26,
        "Test gap ≥ 0.8?",
        "Model → degree 0.45\n0.45 ≥ 0.8 → false",
        MINT,
        "model",
        12.6,
    )
    either = card(ax, 64, 9, 14, 25, "OR", "true OR false\n→ true", PEACH, "logic", 16)
    final = card(
        ax, 84, 43, 13, 26, "AND", "true AND true\n→ report", PINK, "result", 16
    )
    connect(ax, risk, risk_cut)
    connect(ax, risk_cut, either)
    connect(ax, tests, either, "bottom", "top", bend=-0.1)
    connect(ax, either, final, "right", "bottom", bend=0.12)
    connect(ax, fact, final)
    save(fig, "code-review-combinations")


def hierarchical_combinations():
    role = min(0.9, 0.8, 0.7)
    pattern = max(0.6, 0.3)
    semantic = (2 * role + 0.8) / 3
    overall = min(pattern, semantic)
    fig, ax = canvas(6.7)
    heading(
        ax, "Combine combinations", "Different subrules can have different policies"
    )
    role_node = card(
        ax,
        3,
        44,
        26,
        26,
        f"Role fit → {role:.2f}",
        "min(actor 0.9, action 0.8,\ntarget 0.7)",
        BLUE,
        "logic",
        14,
    )
    pattern_node = card(
        ax,
        3,
        9,
        26,
        26,
        f"Pattern fit → {pattern:.2f}",
        "max(conditional 0.6,\nstate 0.3)",
        MINT,
        "logic",
        14,
    )
    semantic_node = card(
        ax,
        36,
        44,
        27,
        26,
        f"Semantic fit → {semantic:.3f}",
        "mean(role 0.7, link 0.8)\nwith weights 2:1",
        PEACH,
        "logic",
        12.8,
    )
    overall_node = card(
        ax,
        71,
        27,
        26,
        26,
        f"Overall → {overall:.2f}",
        "min(pattern, semantic)",
        PINK,
        "logic",
        14,
    )
    connect(ax, role_node, semantic_node)
    connect(ax, semantic_node, overall_node, bend=-0.15)
    connect(ax, pattern_node, overall_node, bend=0.08)
    pill(
        ax,
        43,
        5,
        54,
        12,
        f"Needs review? {overall:.2f} < 0.75 → true",
        color=PAPER,
        size=13,
        bold=True,
    )
    arrow = FancyArrowPatch(
        overall_node.anchor("bottom"),
        (84, 18),
        arrowstyle="-|>",
        mutation_scale=18,
        color=INK,
        linewidth=2,
        zorder=1,
    )
    arrow.set_path_effects(
        [effects.Stroke(linewidth=4.5, foreground=HALO), effects.Normal()]
    )
    ax.add_patch(arrow)
    save(fig, "hierarchical-combinations")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--preview-dir", type=Path, help="Write light/dark inspection previews"
    )
    args = parser.parse_args()
    PREVIEW_DIR = args.preview_dir
    if PREVIEW_DIR is not None:
        PREVIEW_DIR.mkdir(parents=True, exist_ok=True)
    for render in [
        evaluation_flow,
        logic_flow,
        degree_combiners,
        multi_layer_requirements,
        code_review_combinations,
        hierarchical_combinations,
    ]:
        render()
