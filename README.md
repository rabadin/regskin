# Regskin

Regskin is a minimalist web UI for docker registries.
Its goal is to:
- Provide a tree-like view of the content of a docker registry;
- Be reasonably fast (by caching the result of expensive requests).


# Quickstart

Replace `https://registry.internal` by the URL of the target registry and run:
```
docker run -p 3000:3000 -e REGSKIN_REGISTRY_URL=https://registry.internal rvba/regskin
# Point your browser at http://127.0.0.1:3000/
```

# Dev build & run

Pre-requisite: a recent version of [Rust](https://www.rust-lang.org).
```
RUST_BACKTRACE=1 REGSKIN_LOG_LEVEL=info REGSKIN_REGISTRY_URL=https://registry.internal cargo run
# Point your browser at http://127.0.0.1:3000/
```
