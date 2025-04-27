## Extended Examples — `plox`

This document walks through real-world usage patterns of plox.

### Typical First Call.

Plot a single numeric field:
- Extracts `x` value from lines containing `om_module`
- Plots a simple time-series graph

<!-- docify::embed!("tests/cmd_tests.rs", cmd_simple) -->

Sample matching line:
```ignore
2020-01-01 00:14:59.000 om_module x=100.10
```

<img src="some-playground/default.png" width="800" />

### Extracting Value Using Regex.

<!-- docify::embed!("tests/cmd_tests.rs", cmd_regex) -->

Plot a regex extracted numeric values:
- Extracts the numeric values using regex
- only lines containing `yam_module` are matched against regex
- Outputs a simple time-series graph containing two lines each corresponding to extracted values

Sample matching line:
```ignore
2020-01-01 00:14:52.000 yam_module y=(107.107107,892.892893)
```

<img src="some-playground/regex.png" width="800" />

### Events Count and Time Differences

<!-- docify::embed!("tests/cmd_tests.rs", cmd_deltas_and_count) -->

This command:
- Plots the time delta between consecutive "SOME_EVENT" appearances (`event-delta`)
- Plots cumulative number of "SOME_EVENT" occurrences over time (`event-count`)
- It uses `points` style for the deltas and `steps` style for the count
- Plots deltas on a logarithmic primary y-axis (y)
- Plots counts on the secondary y-axis (y2)

Sample matching line:
```ignore
2020-01-01 00:16:33.000 foo_module bla bla "SOME_EVENT"
```

<img src="some-playground/deltas.png" width="800" />


### Adding More Panels.

Plot several numeric fields and data lines together to compare them visually. Timestamp format used for log processing
is also customized. Three panels with multiple data lines are plotted.

<!-- docify::embed!("tests/cmd_tests.rs", cmd_simple_panels) -->

Sample matching lines:
```ignore
[1577834145] foo_module x="SOME_EVENT"
[1577834154] x_module x00=95.50 x01=105.50 x02=115.50 x03=125.50 x04=135.50 x05=145.50 x06=155.50 x07=165.50 x08=175.50 x09=185.50 x10=195.50
[1577834174] om_module x=25.03
```

<img src="some-playground/panels.png" width="800" />

### 🧩 Comparing Multiple Logs Side-by-Side

One of `plox`'s most powerful features is the ability to **compare multiple log files on the same graph layout**.

<!-- docify::embed!("tests/cmd_tests.rs", cmd_simple_panels_two_files) -->

This command:
- This uses the same graph structure as described in the previous section, the same set of panels and plots is applied
- Two log files (`default.log` and `default-other.log`) are provided as input
- `--per-file-panels` automatically duplicates the panel layout once per input file

<img src="some-playground/panels-two-files.png" width="800" />

Similarly, a graph config file can be re-used with many log files:
<!-- docify::embed!("tests/cmd_tests.rs", cmd_demo_lines_two_files) -->

<img src="some-playground/demo-lines-two-files.png" width="800" />


### Using a Toml Graph Config.

Draw a graph defined in [`demo lines`](some-playground/demo-lines.toml) TOML file.

<!-- docify::embed!("tests/cmd_tests.rs", cmd_demo_lines) -->

<img src="some-playground/demo-lines.png" width="800" />

