---
name: plox
description: Plot timestamped logs as graphs. Use when user wants to visualize log data, plot numeric values over time, count events, track time deltas between events, compare multiple log files, or get statistics from logs.
---

# plox - Time Plots from Logs

Turn messy logs into clean graphs. Extract numeric values using regex and plot them over time.

## Requirements

- `plox` must be available in PATH
- `gnuplot` required for PNG output

## Commands

### graph - Plot data from logs

```bash
plox graph --input <LOG_FILE> --plot <GUARD> <FIELD> [OPTIONS]
```

**Basic example:**
```bash
plox graph --input app.log --plot duration
```

**With regex extraction:**
```bash
plox graph --input app.log --plot worker "took:([\d\.]+)(\w+)?"
```

**Multiple panels:**
```bash
plox graph --input app.log \
  --plot module1 value1 \
  --panel \
  --plot module2 value2
```

### stat - Show statistics and histogram

```bash
plox stat --input <LOG_FILE> field-value <GUARD> <FIELD>
```

Shows count, min, max, mean, median, percentiles (q75, q90, q95, q99) and ASCII histogram.

### cat - Display extracted values

```bash
plox cat --input <LOG_FILE> field-value <GUARD> <FIELD>
```

Prints raw extracted timestamp-value pairs.

### match-preview - Debug regex patterns

```bash
plox match-preview --input <LOG_FILE> --verbose <GUARD> <FIELD>
```

Test regex patterns before plotting. Use `-v` or `-vv` for more detail.

## Data Sources (Line Types)

| Option | Description |
|--------|-------------|
| `--plot <guard> <field>` | Plot numeric field values |
| `--event <guard> <pattern> <yvalue>` | Mark events with fixed Y value |
| `--event-count <guard> <pattern>` | Cumulative event count over time |
| `--event-delta <guard> <pattern>` | Time delta between consecutive events |
| `--field-value-sum <guard> <field>` | Cumulative sum of field values |

## Line Styling Options

| Option | Values |
|--------|--------|
| `--style` | `points`, `steps`, `lines`, `lines-points` |
| `--line-color` | red, blue, dark-green, purple, cyan, goldenrod, brown, olive, navy, violet, coral, salmon, steel-blue, dark-magenta, dark-cyan, orange, green, black, magenta, yellow |
| `--line-width` | numeric |
| `--dash-style` | solid, dashed, dotted, dash-dot, long-dash |
| `--marker-type` | dot, triangle-filled, square-filled, diamond-filled, plus, cross, circle, x, triangle, square, diamond |
| `--marker-color` | same as line-color |
| `--marker-size` | numeric (default: 2) |
| `--yaxis` | `y` (primary/left), `y2` (secondary/right) |
| `--title` | Legend label for this line |

## Panel Options

| Option | Description |
|--------|-------------|
| `--panel` | Start a new panel |
| `--panel-title <TITLE>` | Panel title |
| `--height <RATIO>` | Height ratio relative to other panels |
| `--yaxis-scale` | `linear` or `log` |
| `--legend` | `true` or `false` |
| `--time-range-mode` | `full` (union) or `best-fit` (overlap) |

## Input Options

| Option | Description |
|--------|-------------|
| `-i, --input <FILES>` | Log files (comma-separated) |
| `-r, --timestamp-format <FMT>` | Timestamp format (default: `%Y-%m-%d %H:%M:%S%.3f`) |
| `-t, --ignore-invalid-timestamps` | Skip lines with bad timestamps |
| `--guard <GUARDS>` | Global filter - only lines containing all guards |
| `-c, --config <FILE>` | Load TOML config |

## Output Options

| Option | Description |
|--------|-------------|
| `-o, --output <FILE>` | Output PNG path (default: `graph.png`) |
| `-w, --write-config <FILE>` | Save config to TOML |
| `-x, --do-not-display` | Don't open the output file |
| `-p, --plotly-backend` | Generate interactive HTML instead of PNG |
| `--inline-output <FILE>` | Output next to input log file |
| `-a, --display-absolute-paths` | Show absolute paths in output |

**Generated files:**
- PNG graph at specified location
- `.gnuplot` script alongside PNG
- CSV cache in `.plox/` directory next to log files

## Multi-File Comparison

### Binding lines to specific files

```bash
# Apply line only to 3rd input file (0-indexed)
--input a.log,b.log,c.log --plot guard duration --file-id 2

# Apply line to a specific file
--plot guard duration --file-name errors.log
```

### Per-file panel duplication

```bash
# Duplicate panel layout for each input file
plox graph --input a.log,b.log --per-file-panels \
  --plot worker duration
```

Creates separate panels for each log file, useful for side-by-side comparison.

### Panel alignment

| Option | Description |
|--------|-------------|
| `--panel-alignment-mode shared-full` | All panels share same x-axis range (union) |
| `--panel-alignment-mode per-panel` | Each panel has its own x-axis range |
| `--panel-alignment-mode shared-overlap` | Shared range based on overlap |
| `--time-range <RANGE>` | Override with fixed range (for zooming) |

## Timestamp Formats

| Format | Example |
|--------|---------|
| `%Y-%m-%d %H:%M:%S%.3f` | 2025-04-03 11:32:48.027 |
| `%Y-%m-%dT%H:%M:%S%.6fZ` | 2025-06-10T12:08:41.600447Z |
| `[%s]` | [1577834199] |
| `%s` | 1577834199 |
| `%b %d %I:%M:%S %p` | Apr 20 08:26:13 AM |

## Field Regex Patterns

The field can be a simple name or regex with capture groups:

| Pattern | Matches | In Log Line |
|---------|---------|-------------|
| `duration` | `5s` | `duration=5s` |
| `took:([\d\.]+)(\w+)?` | value + unit | `took:5ms` |
| `txs=\((\d+),\s+\d+\)` | first number | `txs=(99,124)` |
| `txs=\(\d+,\s+(\d+)\)` | second number | `txs=(99,124)` |

**Unit conversion:** Time units (s, ms, us, ns) are auto-converted to milliseconds when captured.

## TOML Config Example

```toml
[[panels]]
panel_title = "Metrics"
legend = true

[[panels.lines]]
guard = "worker"
field = "duration"
style = "points"
marker_size = 3.0
marker_color = "red"
title = "Worker duration"

[[panels]]

[[panels.lines]]
guard = "module"
field = 'count=(\d+)'
style = "steps"
line_color = "blue"
```

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `PLOX_IMAGE_VIEWER` | Image viewer for PNG output |
| `PLOX_BROWSER` | Browser for Plotly HTML output |
| `PLOX_SKIP_GNUPLOT` | Skip PNG generation, only save gnuplot script |

## Workflow

1. **Start simple:** `plox graph --input app.log --plot <keyword>`
2. **Debug regex:** `plox match-preview -v --input app.log <guard> <field>`
3. **Check distribution:** `plox stat --input app.log field-value <guard> <field>`
4. **Iterate:** Add panels, styling, more lines
5. **Save config:** `-w config.toml` when CLI gets complex
6. **Reuse:** `plox graph -i new.log -c config.toml`
7. **Compare logs:** `--input a.log,b.log --per-file-panels`

## Real-World Example

Given log lines like:
```
2025-04-22 09:31:00.885 INFO maintain txs=(29382, 0) duration=56.206398ms
2025-04-22 09:31:13.081 DEBUG prune: validated_counter=2, took:4.708552ms
```

```bash
# Plot prune duration and validation count
plox graph --input eve.log \
  --plot prune "validated_counter" --style points --marker-size 3 \
  --panel \
  --plot prune "took:([\d\.]+)(\w+)?" --style points --marker-size 3

# Extract watched txs count from maintain lines
plox graph --input eve.log \
  --plot maintain "txs=\((\d+),\s+\d+\)"
```
