### `plox` CLI reference:
```ignore
Turn messy logs into clean graphs. Plot fields or regex matches over time, mark events, count occurrences — all from your terminal.

Usage: plox [OPTIONS] <COMMAND>

Commands:
  stat           Display stats and histogram for extracted data
  cat            Display extracted values only
  graph          Extract and plot structured data from logs.
  match-preview  Test regex field patterns on log files before plotting
  help           Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose...
          Global verbosity (-v , -vv)
          
          Levels: - info enabled by default - -v for debug - -vv for trace

  -q, --quiet
          Quiet mode, no output

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```
---
### `plox graph` reference:
```ignore
The 'graph' command parses timestamped log files and plots numeric fields, regex captures, events, or deltas over time.

Supports:
- Regex-based value extraction,
- Named fields with optional guards,
- Multiple panels and file-aware layouts.


Usage: plox graph [OPTIONS]

Options:
  -h, --help
          Print help (see a summary with '-h')

Data sources - plotted line types:
  --event <guard> <pattern> <yvalue>
          Plot a fixed numerical value (`yvalue`) whenever `pattern` appears in logs
            <guard>: Optional guard string to quickly filter out log lines using `strcmp`
            <pattern>: Substring or regex pattern to match in log lines
            <yvalue>: The fixed value to plot each time `pattern` is found
          

  --event-count <guard> <pattern>
          Plot a cumulative count of `pattern` occurrences over time
            <guard>: Optional guard string to quickly filter out log lines using `strcmp`
            <pattern>: Substring or regex pattern to match in log lines
          

  --event-delta <guard> <pattern>
          Plot the time delta between consecutive occurrences of `pattern`
            <guard>: Optional guard string to quickly filter out log lines using `strcmp`
            <pattern>: Substring or regex pattern to match in log lines
          

  --plot <guard> <field>
          Plot a numeric field from logs
            <guard>: Optional guard string to quickly filter out log lines using `strcmp`
            <field>: The name of the field to parse as numeric or regex. Refer to "Plot Field Regex" help section for more details
          

Line Options:
  --file-name <FILE_NAME>
          Optionally overrides source log file.
          
          Assigns a specific file to the line

  --file-id <FILE_ID>
          Optionally specifies the index of input file.
          
          Assigns the line to the nth input from `--input` (index starting at 0)

  --title <TITLE>
          Optional title of the line. Will be placed on legend

  --style <STYLE>
          The style of the plotted line
          
          [default: points]
          [possible values: points, steps, lines-points, lines]

  --line-width <LINE_WIDTH>
          The width of the line

  --line-color <LINE_COLOR>
          The color of the line
          
          [possible values: red, blue, dark-green, purple, cyan, goldenrod, brown, olive, navy, violet, coral, salmon, steel-blue, dark-magenta, dark-cyan, dark-yellow, dark-turquoise, yellow, black, magenta, orange, green, dark-orange]

  --dash-style <DASH_STYLE>
          The dash type
          
          [possible values: solid, dashed, dotted, dash-dot, long-dash]

  --yaxis <YAXIS>
          Which Y-axis this line should use, if applicable (e.g. primary or secondary)

          Possible values:
          - y:  Primary Y-axis (the left side)
          - y2: Secondary Y-axis (the right side)

  --marker-type <MARKER_TYPE>
          The marker type
          
          [possible values: dot, triangle-filled, square-filled, diamond-filled, plus, cross, circle, x, triangle, square, diamond]

  --marker-color <MARKER_COLOR>
          The color of the marker (if markers are enabled)
          
          [possible values: red, blue, dark-green, purple, cyan, goldenrod, brown, olive, navy, violet, coral, salmon, steel-blue, dark-magenta, dark-cyan, dark-yellow, dark-turquoise, yellow, black, magenta, orange, green, dark-orange]

  --marker-size <MARKER_SIZE>
          The size of the marker
          
          [default: 2]

Panel Options:
  --panel-title <PANEL_TITLE>
          Title displayed above the panel

  --height <HEIGHT>
          Height ratio (relative to other panels)

  --yaxis-scale <YAXIS_SCALE>
          Y-axis scale (linear or log)
          
          [possible values: linear, log]

  --legend <LEGEND>
          Show legend.
          
          Legend will be shown if not provided.
          
          [possible values: true, false]

  --time-range-mode <TIME_RANGE_MODE>
          Panel range mode.
          
          How panel time range shall be generated.

          Possible values:
          - full:     Use the full span of all line ranges (min start, max end)
          - best-fit: Use the overlapping time window of all lines (max start, min end)

  --panel
          Add new panel to graph

Input files:
  -i, --input <INPUT>
          Input log files to be processed. Comma-separated list of input log files to be processed

      --timestamp-format <TIMESTAMP_FORMAT>
          The format of the timestamp which is used in logs.
          
          For exact format specifiers refer to: <https://docs.rs/chrono/latest/chrono/format/strftime/index.html>
          
          [default: '%Y-%m-%d %H:%M:%S%.3f']

  -t, --ignore-invalid-timestamps
          Do not fail if log contains lines with invalid timestamp.
          
          Ignores invalid timestamps. Useful when log contains line with invalid or no timestamp (e.g. stacktraces).

  -c, --config <FILE>
          Path to TOML config file containing panels layout.

Output files:
      --cache-dir <DIR>
          Directory to store parsed CSV cache files. The full path of each log file is mirrored inside this directory to avoid name collisions. If not set, a `.plox/` directory is created next to each log file to store its cache

  -f, --force-csv-regen
          Forces regeneration of the CSV cache by re-parsing the log files

  -w, --write-config <CONFIG-FILE>
          Additionally writes the current graph configuration to a file in TOML format

  -o, --output <FILE>
          Path to the output PNG graph file.
          
          The corresponding `.gnuplot` script will be written alongside it, using the same filename with a different extension. Ignored if `--inline-output` is set.
          
          If nothing is provided `graph.png` and `graph.gnuplot` in current directory will be stored.

      --inline-output <FILE>
          Output filename to be placed in a location derived from the input log file paths.
          
          Location of file is automatically resolved as follow: - If a single log file is provided, the output goes next to it. - If multiple log files are used, the output goes to their common ancestor directory.
          
          This option is a convenience shortcut: only the directory is inferred — the filename must be provided here.
          
          Overrides `--output` if both are set.

  -a, --display-absolute-paths
          Indicates if absolute paths to output files shall be displayed.
          
          Otherwise relative path will be displayed.

  -x, --do-not-display
          Do not display the graph in the image viewer.
          
          Suppresses launching the system image viewer (or browser for Plotly) to display the output. Viewers can be configured via `PLOX_IMAGE_VIEWER` or `PLOX_BROWSER` environment variables.

Panels layout:
  --per-file-panels [<PER_FILE_PANELS>]
          When enabled, creates a separate panel for each input file.
          
          If any panel contains lines that are not explicitly bound to a file (i.e. no `file_name` or `file_id` set), that panel will be duplicated once per input file. Each duplicated panel will contain lines resolved to a specific file from the input list.
          
          Panels and lines that already target specific files are unaffected by this option.
          
          [possible values: true, false]

  --panel-alignment-mode <PANEL_ALIGNMENT_MODE>
          Strategy for aligning time ranges across all panels.
          
          This determines how time-axis (x) ranges are handled when plotting.
          
          [possible values: shared-full, per-panel, shared-overlap]

  --time-range <TIME_RANGE>
          Optional override for the global time range used in the graph.
          
          Can be specified as either: - A relative range in `[0.0, 1.0]`, - Two timestamp strings.
          
          Timestamp strings must be compatible with the `--timestamp-format`.
          
          Conflicts with `--panel-alignment-mode`, and implies global alignment.

Backend:
  -p, --plotly-backend
          Use plotly backend, generated interactive self-contained html file

Environment variables:
There are two environment variables controlling behaviour of graph command:
- `PLOX_IMAGE_VIEWER` - the name (or path) of the executable that will be used to display image generated by `gnuplot`.
- `PLOX_BROWSER` - the name (or path) of the executable that will be used to display html generated by plotly backend.
- `PLOX_SKIP_GNUPLOT` - if set, the gnuplot image generation will not be executed, only gnuplot script will be saved.

Line matching:
- Firstly, if an expression is provided by the user, the guard is used to quickly filter out non-matching lines by comparing it with the line using strcmp.
- Secondly, the timestamp pattern is used to extract the timestamp.
- Thirdly, the field/pattern regex is applied.

Try `plox match-preview --verbose` to debug matching issues.

Timestamp format:
The tool is designed to parse timestamped logs. The timestamp format used in the log file shall be passed as the `--timestamp-format` parameter.

For the the exact format specifiers refer to: https://docs.rs/chrono/latest/chrono/format/strftime/index.html

Examples:
- "2025-04-03 11:32:48.027"  | "%Y-%m-%d %H:%M:%S%.3f"
- "08:26:13 AM"              | "%I:%M:%S %p"
- "2025 035 08:26:13 AM"     | "%Y %j %I:%M:%S %p"
- "035 08:26:13 AM"          | "%j %I:%M:%S %p"
- "[1577834199]"             | "[%s]"
- "1577834199"               | "%s"
- "Apr 20 08:26:13 AM"       | "%b %d %I:%M:%S %p"
- "[100.333]"                | not supported...

Field regex:
Regex pattern shall contain a single capture group for matching value only, or two
capture groups for matching value and unit.

Currently only time-related units are implemented (s,ms,us,ns) and all values are converted to miliseconds.
If catpure group for units is not provided, no conversion is made.

Regex pattern does not match the timestamp. Timestamp will be striped and the remainder
for the log line will matched against regex.

Examples:
- "duration"                       | matches "5s" in "duration=5s"
- "\bduration:([\d\.]+)(\w+)?"     | matches "5s" in log: "duration:5s"
- "\bvalue:([\d\.]+)?"             | matches "75" in log: "value:75" (no units)
- "^\s+(?:[\d\.]+\s+){3}([\d\.]+)" | matches 4th column (whitespace separated)
- "txs=\(\d+,\s+(\d+)\)"           | matches '124' in "txs=(99,124)
```
