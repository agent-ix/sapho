#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Agent-IX
"""Render original Sapho documentation diagrams with Matplotlib (no model calls).

Run from any directory with Python and Matplotlib installed:
    MPLCONFIGDIR=/tmp/sapho-docs-mpl python3 docs/images/generate.py
PNG is used in Markdown; SVG is retained for scaling and reuse.
"""

from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.patches import FancyArrowPatch, FancyBboxPatch


OUT = Path(__file__).resolve().parent
INK = "#172e3b"
MUTED = "#516472"
BLUE = "#2477a6"
TEAL = "#16816b"
GOLD = "#a56b12"
PALE = "#f5f8fa"

plt.rcParams.update({
    "font.family": "DejaVu Sans",
    "font.size": 12,
    "text.color": INK,
    "svg.fonttype": "none",
    "svg.hashsalt": "sapho-docs",
    "savefig.facecolor": "white",
})


def canvas(width, height):
    fig, ax = plt.subplots(figsize=(width, height), facecolor="white")
    ax.set_xlim(0, 100)
    ax.set_ylim(0, 100)
    ax.axis("off")
    fig.subplots_adjust(left=0.015, right=0.985, bottom=0.025, top=0.975)
    return fig, ax


def box(ax, x, y, width, height, title, detail, color=BLUE):
    ax.add_patch(FancyBboxPatch(
        (x, y), width, height,
        boxstyle="round,pad=0.55,rounding_size=1.8",
        facecolor=PALE, edgecolor=color, linewidth=1.6,
    ))
    ax.text(x + width / 2, y + height * 0.69, title,
            ha="center", va="center", fontsize=13, fontweight="bold", color=color)
    ax.text(x + width / 2, y + height * 0.30, detail,
            ha="center", va="center", fontsize=10.5, linespacing=1.5)


def arrow(ax, start, end, color=MUTED, style="-"):
    ax.add_patch(FancyArrowPatch(
        start, end, arrowstyle="-|>", mutation_scale=16,
        linewidth=1.6, color=color, linestyle=style,
    ))


def save(fig, name):
    fig.savefig(OUT / f"{name}.png", dpi=160,
                metadata={
                    "Software": "Sapho documentation renderer",
                    "Copyright": "Copyright (C) 2026 Agent-IX",
                    "License": "SPDX-License-Identifier: AGPL-3.0-or-later",
                })
    fig.savefig(OUT / f"{name}.svg", metadata={
        "Date": None,
        "Creator": "Sapho documentation renderer",
        "Rights": "SPDX-License-Identifier: AGPL-3.0-or-later; Copyright (C) 2026 Agent-IX",
    })
    svg = OUT / f"{name}.svg"
    svg.write_text("\n".join(line.rstrip() for line in svg.read_text().splitlines()) + "\n")
    plt.close(fig)


def evaluation_flow():
    fig, ax = canvas(11.5, 5.2)
    ax.text(3, 94, "From judgments to decisions", fontsize=23,
            fontweight="bold", va="center")
    ax.text(3, 84, "Your application defines the rule. Sapho connects and runs it.",
            fontsize=12, color=MUTED)
    box(ax, 3, 43, 20, 28, "Your inputs", "Code units\nRequirements\nContext", BLUE)
    box(ax, 29, 43, 20, 28, "Rust functions", "Extract items\nBuild context\nCompute facts", BLUE)
    box(ax, 55, 43, 20, 28, "Model questions", "Boolean / choice / score\nJev or your backend", TEAL)
    box(ax, 81, 43, 16, 28, "Logic", "Combine evidence\nApply thresholds", GOLD)
    for start, end in [(23.7, 28.2), (49.7, 54.2), (75.7, 80.2)]:
        arrow(ax, (start, 57), (end, 57))
    box(ax, 55, 4, 42, 23, "Outputs + execution trace",
        "Your application presents findings or acts on decisions", TEAL)
    arrow(ax, (89, 42), (89, 28))
    ax.text(3, 22, "Add stages and branches as your rule needs them.",
            fontsize=11, color=MUTED)
    ax.text(3, 12, "TOML graph  ·  typed connections  ·  explicit run limits",
            fontsize=10.5, color=MUTED)
    save(fig, "evaluation-flow")


def logic_flow():
    fig, ax = canvas(11.5, 5.4)
    ax.text(3, 94, "Make each interpretation explicit", fontsize=23,
            fontweight="bold", va="center")
    ax.text(3, 84, "A probability, a heuristic strength and a decision have different meanings.",
            fontsize=11.5, color=MUTED)
    box(ax, 3, 43, 19, 28, "Probability", "Project model\noutcome mass", TEAL)
    box(ax, 28, 43, 19, 28, "Degree", "Interpret evidence\non a 0–1 scale", BLUE)
    box(ax, 53, 43, 22, 28, "Combined degree", "min / max\nweighted mean", GOLD)
    box(ax, 81, 43, 16, 28, "Boolean", "Compare with\na threshold", TEAL)
    for start, end in [(22.7, 27.2), (47.7, 52.2), (75.7, 80.2)]:
        arrow(ax, (start, 57), (end, 57))
    ax.text(25, 37, "degree", ha="center", fontsize=10, color=MUTED)
    ax.text(50, 37, "reduce", ha="center", fontsize=10, color=MUTED)
    ax.text(78, 37, "compare", ha="center", fontsize=10, color=MUTED)
    box(ax, 29, 3, 27, 23, "Other degree inputs", "Rust facts mapped to strengths\nOther model judgments", BLUE)
    arrow(ax, (56.7, 15), (64, 42))
    ax.text(64, 17, "Example: 0.65 < 0.7 → true", ha="left", fontsize=11,
            fontweight="bold", color=TEAL)
    ax.text(64, 7, "The application decides what true means.",
            ha="left", fontsize=10, color=MUTED)
    save(fig, "logic-flow")


def degree_combiners():
    values = [0.8, 0.4, 0.6]
    weights = [2.0, 1.0, 1.0]
    mean = sum(w * x for w, x in zip(weights, values)) / sum(weights)
    results = [min(values), max(values), mean]
    fig, axes = plt.subplots(1, 2, figsize=(11.5, 5.5), facecolor="white")
    fig.subplots_adjust(left=0.10, right=0.95, top=0.70, bottom=0.20, wspace=0.42)
    fig.text(0.035, 0.91, "Same evidence. Different rule policies.",
             fontsize=23, fontweight="bold")
    fig.text(0.035, 0.83, "Combine strengths of 0.8, 0.4 and 0.6. Weighted mean uses weights 2 : 1 : 1.",
             fontsize=11.5, color=MUTED)
    for ax, labels, data, colors, title in [
        (axes[0], ["Input A", "Input B", "Input C"], values,
         [BLUE] * 3, "Evidence strengths"),
        (axes[1], ["Minimum", "Maximum", "Weighted mean"], results,
         [GOLD, TEAL, BLUE], "Combined strength"),
    ]:
        ax.barh(labels, data, color=colors, height=0.50, zorder=3)
        ax.invert_yaxis()
        ax.set_xlim(0, 1.05)
        ax.set_xticks([0, 0.25, 0.5, 0.75, 1])
        ax.set_xticklabels(["0", "0.25", "0.5", "0.75", "1"])
        ax.set_xlabel("Heuristic degree", labelpad=8, fontsize=11)
        ax.set_title(title, loc="left", pad=14, fontsize=13, fontweight="bold")
        ax.grid(axis="x", color="#e2e8ed", zorder=0)
        ax.tick_params(axis="both", length=0, labelsize=11)
        for spine in ax.spines.values():
            spine.set_visible(False)
        for y, value in enumerate(data):
            ax.text(value + 0.025, y, f"{value:.2f}", va="center", fontsize=12,
                    fontweight="bold", color=INK)
    fig.text(0.035, 0.06,
             "Minimum: weakest part   ·   Maximum: strongest part   ·   Weighted mean: balance with chosen weights",
             fontsize=10.5, color=MUTED)
    save(fig, "degree-combiners")


if __name__ == "__main__":
    evaluation_flow()
    logic_flow()
    degree_combiners()
