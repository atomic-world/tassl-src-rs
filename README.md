# tassl-src

This crate is used to build TASSL. This crate only works on Linux & Mac.

# Installation

```toml
[package]
links = "ssl"
build = "build.rs"

[dependencies]
libc = "0.2"

[build-dependencies]
tassl-src = "0.1"
```

Add below codes in your `build.rs`:

```rust
fn main() {
    let artifacts = tassl_src::Builder::default().build();
    artifacts.print_cargo_metadata();
}
```

# License

[Apache License, Version 2.0](./LICENSE)
