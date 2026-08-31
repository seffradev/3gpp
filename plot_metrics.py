#!/usr/bin/env python3
"""
Plot GTP load-test metrics CSVs produced by `utilities/load`.

Expected CSV columns (header row required):
unix_time,sent,received,timeouts,errors,rejected,throughput_per_sec,p50_ms,p90_ms,p99_ms,max_ms

Usage:
    # Single run
    python3 plot_metrics.py baseline-closed.csv

    # Compare multiple runs (e.g. baseline vs proxy), each given a label
    python3 plot_metrics.py --label baseline baseline-closed.csv --label proxy proxy-closed.csv

    # Custom output directory / file prefix
    python3 plot_metrics.py baseline-closed.csv --out-dir ./plots --prefix run1

Requires: pandas, matplotlib
    pip install pandas matplotlib
"""

import argparse
import sys
from pathlib import Path

import pandas as pd
import matplotlib.pyplot as plt

REQUIRED_COLUMNS = [
    "unix_time",
    "sent",
    "received",
    "timeouts",
    "errors",
    "rejected",
    "throughput_per_sec",
    "p50_ms",
    "p90_ms",
    "p99_ms",
    "max_ms",
]


def load_csv(path: Path) -> pd.DataFrame:
    df = pd.read_csv(path)
    missing = [c for c in REQUIRED_COLUMNS if c not in df.columns]
    if missing:
        sys.exit(f"error: {path} is missing expected columns: {missing}")
    df["time"] = pd.to_datetime(df["unix_time"], unit="s")
    # elapsed seconds since the start of this run, handy for overlaying runs of different wall-clock times
    df["elapsed_s"] = df["unix_time"] - df["unix_time"].iloc[0]
    return df


def add_error_rate(df: pd.DataFrame) -> pd.DataFrame:
    df = df.copy()
    total = df["sent"].replace(0, pd.NA)
    df["timeout_rate_pct"] = (df["timeouts"] / total * 100).fillna(0)
    df["error_rate_pct"] = (df["errors"] / total * 100).fillna(0)
    df["rejected_rate_pct"] = (df["rejected"] / total * 100).fillna(0)
    return df


def plot_throughput(runs: dict[str, pd.DataFrame], out_path: Path, use_elapsed: bool):
    fig, ax = plt.subplots(figsize=(11, 5))
    x_col = "elapsed_s" if use_elapsed else "time"
    for label, df in runs.items():
        ax.plot(
            df[x_col], df["throughput_per_sec"], marker="o", markersize=3, label=label
        )
    ax.set_title("Throughput over time")
    ax.set_ylabel("responses / sec")
    ax.set_xlabel("elapsed seconds" if use_elapsed else "time")
    ax.grid(True, alpha=0.3)
    ax.legend()
    if not use_elapsed:
        fig.autofmt_xdate()
    fig.tight_layout()
    fig.savefig(out_path, dpi=150)
    plt.close(fig)


def plot_latency(runs: dict[str, pd.DataFrame], out_path: Path, use_elapsed: bool):
    fig, axes = plt.subplots(2, 2, figsize=(13, 8), sharex=True)
    metrics = [
        ("p50_ms", "p50 latency"),
        ("p90_ms", "p90 latency"),
        ("p99_ms", "p99 latency"),
        ("max_ms", "max latency"),
    ]
    x_col = "elapsed_s" if use_elapsed else "time"

    for ax, (col, title) in zip(axes.flat, metrics):
        for label, df in runs.items():
            ax.plot(df[x_col], df[col], marker="o", markersize=3, label=label)
        ax.set_title(title)
        ax.set_ylabel("ms")
        ax.grid(True, alpha=0.3)

    axes[0, 0].legend()
    for ax in axes[-1]:
        ax.set_xlabel("elapsed seconds" if use_elapsed else "time")
    if not use_elapsed:
        fig.autofmt_xdate()
    fig.tight_layout()
    fig.savefig(out_path, dpi=150)
    plt.close(fig)


def plot_latency_overlay(
    runs: dict[str, pd.DataFrame], out_path: Path, use_elapsed: bool
):
    """All latency percentiles for all runs on one chart, p50/p90/p99 as line styles."""
    fig, ax = plt.subplots(figsize=(12, 6))
    x_col = "elapsed_s" if use_elapsed else "time"
    styles = {"p50_ms": "-", "p90_ms": "--", "p99_ms": ":"}

    color_cycle = plt.rcParams["axes.prop_cycle"].by_key()["color"]
    for i, (label, df) in enumerate(runs.items()):
        color = color_cycle[i % len(color_cycle)]
        for col, style in styles.items():
            ax.plot(
                df[x_col],
                df[col],
                style,
                color=color,
                label=f"{label} {col.replace('_ms', '')}",
            )

    ax.set_title("Latency percentiles (p50 solid / p90 dashed / p99 dotted)")
    ax.set_ylabel("ms")
    ax.set_xlabel("elapsed seconds" if use_elapsed else "time")
    ax.grid(True, alpha=0.3)
    ax.legend(fontsize=8, ncol=2)
    if not use_elapsed:
        fig.autofmt_xdate()
    fig.tight_layout()
    fig.savefig(out_path, dpi=150)
    plt.close(fig)


def plot_error_rates(runs: dict[str, pd.DataFrame], out_path: Path, use_elapsed: bool):
    fig, ax = plt.subplots(figsize=(11, 5))
    x_col = "elapsed_s" if use_elapsed else "time"
    color_cycle = plt.rcParams["axes.prop_cycle"].by_key()["color"]

    for i, (label, df) in enumerate(runs.items()):
        color = color_cycle[i % len(color_cycle)]
        ax.plot(
            df[x_col],
            df["timeout_rate_pct"],
            "-",
            color=color,
            label=f"{label} timeout%",
        )
        ax.plot(
            df[x_col], df["error_rate_pct"], "--", color=color, label=f"{label} error%"
        )
        ax.plot(
            df[x_col],
            df["rejected_rate_pct"],
            ":",
            color=color,
            label=f"{label} rejected%",
        )

    ax.set_title("Timeout / error / rejection rate over time")
    ax.set_ylabel("% of sent requests")
    ax.set_xlabel("elapsed seconds" if use_elapsed else "time")
    ax.grid(True, alpha=0.3)
    ax.legend(fontsize=8, ncol=3)
    if not use_elapsed:
        fig.autofmt_xdate()
    fig.tight_layout()
    fig.savefig(out_path, dpi=150)
    plt.close(fig)


def print_summary_table(runs: dict[str, pd.DataFrame]):
    print(
        "\n=== Summary (mean across intervals; p99/max are the worst interval seen) ==="
    )
    header = f"{'run':<20}{'average_throughput':>10}{'p50_ms':>9}{'p90_ms':>9}{'p99_ms':>9}{'max_ms':>9}{'timeouts':>10}{'errors':>9}{'rejected':>10}"
    print(header)
    print("-" * len(header))
    for label, df in runs.items():
        print(
            f"{label:<20}"
            f"{df['throughput_per_sec'].mean():>18.1f}"
            f"{df['p50_ms'].mean():>9.2f}"
            f"{df['p90_ms'].mean():>9.2f}"
            f"{df['p99_ms'].max():>9.2f}"
            f"{df['max_ms'].max():>9.2f}"
            f"{df['timeouts'].sum():>10}"
            f"{df['errors'].sum():>9}"
            f"{df['rejected'].sum():>10}"
        )
    print()


def main():
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "files",
        nargs="*",
        help="CSV file(s) to plot (unlabeled; filename stem used as label)",
    )
    parser.add_argument(
        "--label",
        nargs=2,
        action="append",
        metavar=("NAME", "FILE"),
        help="Add a labeled run, e.g. --label proxy proxy-run.csv. Can repeat.",
    )
    parser.add_argument(
        "--out-dir",
        default=".",
        help="Directory to write PNGs into (default: current dir)",
    )
    parser.add_argument(
        "--prefix", default="gtp_metrics", help="Filename prefix for output PNGs"
    )
    parser.add_argument(
        "--elapsed",
        action="store_true",
        help="Use elapsed seconds since each run's start on the x-axis instead of wall-clock time "
        "(recommended when comparing runs that were started at different times)",
    )
    args = parser.parse_args()

    runs: dict[str, pd.DataFrame] = {}

    for f in args.files:
        path = Path(f)
        runs[path.stem] = load_csv(path)

    if args.label:
        for name, f in args.label:
            path = Path(f)
            runs[name] = load_csv(path)

    if not runs:
        parser.error("no input files given (pass CSV paths, or use --label NAME FILE)")

    runs = {label: add_error_rate(df) for label, df in runs.items()}

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    use_elapsed = (
        args.elapsed or len(runs) > 1
    )  # default to elapsed when comparing multiple runs

    plot_throughput(runs, out_dir / f"{args.prefix}_throughput.png", use_elapsed)
    plot_latency(runs, out_dir / f"{args.prefix}_latency_grid.png", use_elapsed)
    plot_latency_overlay(
        runs, out_dir / f"{args.prefix}_latency_overlay.png", use_elapsed
    )
    plot_error_rates(runs, out_dir / f"{args.prefix}_error_rates.png", use_elapsed)

    print_summary_table(runs)
    print(f"Wrote plots to {out_dir.resolve()}/{args.prefix}_*.png")


if __name__ == "__main__":
    main()
