# Trident Syscall Stubs

This crate provides a set of syscall stubs for the Trident SVM.

> [!WARNING]
> - The crate `trident-syscall-stubs-v1` is meant to be used with programs written in Solana >=1.17 < 2
>
> - The crate `trident-syscall-stubs-v2` is meant to be used with programs written in Solana ~2.0.

## Usage

Add this dependency to your `Cargo.toml`:


```toml
[dependencies]
trident-syscall-stubs-v1 = "0.0.1"
trident-syscall-stubs-v2 = "0.0.1"
```

or

```toml
[dependencies.trident-syscall-stubs-v1]
git = "https://github.com/Ackee-Blockchain/trident-syscall-stubs"
package = "trident-syscall-stubs-v1"

[dependencies.trident-syscall-stubs-v2]
git = "https://github.com/Ackee-Blockchain/trident-syscall-stubs"
package = "trident-syscall-stubs-v2"
```
