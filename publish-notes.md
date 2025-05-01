### Steps to publish

- Make sure it builds + tests
```bash
cargo build --release
cargo test --release
```

- rebuild doc:
```bash
cargo build --features=generate-readme
```

- bump version of `plox_macros` it if needed:
- bump version of `plox` 

- open PR.
- publish `plox_macros` if needed:
```bash
cd plox_macros
cargo publish -n
cargo publish 
```

- bump version of `plox` and publish it:
```bash
cargo publish -n
cargo publish 
```

- merge + tag
```
git tag v0.5.0
git push origin v0.5.0
```
