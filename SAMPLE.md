### Typical first call.

Some very basic usage. Just plot some value from the log:

```rust,ignore
plox graph \
	  --input  some-playground/default.log \
	  --output some-playground/default.png \
	  --plot down x

```

<img src="https://raw.githubusercontent.com/michalkucharczyk/plox/readme_examples/some-playground/default.png" width="800" />

### Adding more panels.

- timestamp format customized,
- some panels added

```rust,ignore
plox graph \
	  --input  some-playground/some.log \
	  --output some-playground/panels.png \
	  --timestamp-format "[%s]" \
	  --plot down x \
	  --panel \
	  --plot linear x01 \
	  --plot linear x02 \
	  --plot linear x03 \
	  --panel \
	  --event-count event SOME_EVENT \
	  --event event SOME_EVENT 1.0 --yaxis y2 --style points

```

<img src="https://raw.githubusercontent.com/michalkucharczyk/plox/readme_examples/some-playground/panels.png" width="800" />

### Using a toml graph config.

Draw a graph defined in [`demo lines`](some-playground/demo-lines.toml) TOML file.

```rust,ignore
plox graph \
	  --input  some-playground/some.log \
	  --output some-playground/demo-lines.png \
	  --config some-playground/demo-lines.toml

```

<img src="https://raw.githubusercontent.com/michalkucharczyk/plox/readme_examples/some-playground/demo-lines.png" width="800" />

