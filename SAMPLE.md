## Extended Examples — `plox`

This document walks through real-world usage patterns of plox.

### Typical First Call.

Plot a single numeric field:
- Extracts `x` value from lines containing `om_module`
- Plots a simple time-series graph

```rust,ignore
plox graph \
	  --input  tests/examples/default.log \
	  --output tests/.output/default.png \
	  --plot om_module x

```

Sample matching line ([`log`](tests/examples/default.log)):
```ignore
2020-01-01 00:14:59.000 om_module x=100.10
```

<img src="tests/examples/default.png" width="800" />

---

### Interactive plots - Plotly Backend

To generate an interactive HTML plot use the `--plotly-backend` option:

```rust,ignore
plox graph \
	  --input  tests/examples/default.log \
	  --output tests/.output/default.html \
	  --plot om_module x \
	  --style=lines-points \
	  --plotly-backend

```

Here is a [simple interactive graph](https://michalkucharczyk.github.io/plox/tests/examples/default.html) and a more
[complex one](https://michalkucharczyk.github.io/plox/tests/examples/demo-lines-two-files.html).

---


### Extracting Value Using Regex.

```rust,ignore
plox graph \
	  --input  tests/examples/default.log \
	  --output tests/.output/regex.png \
	  --plot yam_module "y=\([\d\.]+,\s*([\d\.]+)\)" \
	  --title "1st tuple item" \
	  --plot yam_module "y=\(([\d\.]+),\s*[\d\.]+\)" \
	  --title "2nd tuple item"

```

Plot a regex extracted numeric values:
- Extracts the numeric values using regex
- only lines containing `yam_module` are matched against regex
- Outputs a simple time-series graph containing two lines each corresponding to extracted values

Sample matching line ([`log`](tests/examples/default.log)):
```ignore
2020-01-01 00:14:52.000 yam_module y=(107.107107,892.892893)
```

<img src="tests/examples/regex.png" width="800" />

---

### Events Count and Time Differences

```rust,ignore
plox graph \
	  --input tests/examples/default.log \
	  --output tests/.output/deltas.png \
	  --event-delta foo_module "SOME_EVENT" --yaxis-scale log \
	  --style points --marker-size 7 --marker-color olive --marker-type diamond \
	  --event-count foo_module "SOME_EVENT" --style steps --yaxis y2

```

This command:
- Plots the time delta between consecutive `SOME_EVENT` appearances (`event-delta`)
- Plots cumulative number of `SOME_EVENT` occurrences over time (`event-count`)
- It uses `points` style for the deltas and `steps` style for the count
- Plots deltas on a logarithmic primary y-axis (y)
- Plots counts on the secondary y-axis (y2)

Sample matching line ([`log`](tests/examples/default.log)):
```ignore
2020-01-01 00:16:33.000 foo_module bla bla "SOME_EVENT"
```

<img src="tests/examples/deltas.png" width="800" />

---

### Adding More Panels.

Plot several numeric fields and data lines together to compare them visually. Timestamp format used for log processing
is also customized. Three panels with multiple data lines are plotted.

```rust,ignore
plox graph \
	  --input  tests/examples/some.log \
	  --output tests/.output/panels.png \
	  --timestamp-format "[%s]" \
	  --plot om_module x \
	  --panel \
	  --plot x_module x01 \
	  --plot x_module x02 \
	  --plot x_module x03 \
	  --panel \
	  --event-count foo_module SOME_EVENT \
	  --event foo_module SOME_EVENT 1.0 --yaxis y2 --style points

```

Sample matching line ([`log`](tests/examples/some.log)):
```ignore
[1577834145] foo_module x="SOME_EVENT"
[1577834154] x_module x00=95.50 x01=105.50 x02=115.50 x03=125.50 x04=135.50 x05=145.50 x06=155.50 x07=165.50 x08=175.50 x09=185.50 x10=195.50
[1577834174] om_module x=25.03
```

<img src="tests/examples/panels.png" width="800" />

---

### Using a Toml Graph Config.

Draw a graph defined in 👉 [`demo lines`](tests/examples/demo-lines.toml) TOML file.

```rust,ignore
plox graph \
	  --input  tests/examples/some.log \
	  --timestamp-format "[%s]" \
	  --output tests/.output/demo-lines.png \
	  --config tests/examples/demo-lines.toml

```

<img src="tests/examples/demo-lines.png" width="800" />


---

### 🧩 Comparing Multiple Logs Side-by-Side

One of `plox`'s most powerful features is the ability to **compare multiple log files on the same graph layout**.

```rust,ignore
plox graph \
	  --input  tests/examples/default.log,tests/examples/default-other.log \
	  --output tests/.output/panels-two-files.png \
	  --per-file-panels \
	  --plot om_module x \
	  --panel \
	  --plot x_module x01 \
	  --plot x_module x02 \
	  --plot x_module x03 \
	  --panel \
	  --event-count foo_module SOME_EVENT \
	  --event foo_module SOME_EVENT 1.0 --yaxis y2 --style points

```

This command:
- This uses the same graph structure as described in the previous section, the same set of panels and plots is applied
- Two log files (`default.log` and `default-other.log`) are provided as input
- `--per-file-panels` automatically duplicates the panel layout once per input file

<img src="tests/examples/panels-two-files.png" width="800" />

Similarly, a graph config file can be re-used with many log files:
```rust,ignore
plox graph \
	  --input  tests/examples/default.log \
	  --input  tests/examples/default-other.log \
	  --output tests/.output/demo-lines-two-files.png \
	  --timestamp-format "%Y-%m-%d %H:%M:%S%.3f" \
	  --per-file-panels \
	  --config tests/examples/demo-lines.toml

```

<img src="tests/examples/demo-lines-two-files.png" width="800" />
