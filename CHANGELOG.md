## [0.9.0] - 2026-10-13

### 🚀 Features

- Change data structures model, decrease memory usage (#49)

### 🐛 Bug Fixes

- *(deps)* Bonfig for configs
- *(deps)* Update ntex up to 4.1
- Sorting, ranking, and load old index data

### ⚙️ Miscellaneous Tasks

- Flake nix
## [0.8.5] - 2026-10-09

### 🚀 Features

- Kiddo with rkyv_08 feature to serialize whole index

### 🐛 Bug Fixes

- Clippy
- Use rust v1.90 as base docker image
- *(deps)* Use ntex v3 prerelease
- Clippy
- Clippy
- Update ntex
- Clippy fixes
- Reqwest feature
- Add option to configure exclude list of feature codes ref #46
- Remove skip metadata dump write ref #50

### 📚 Documentation

- Run test command

### ⚙️ Miscellaneous Tasks

- Release v0.8.1
- Clippy::if_not_else
- Add justfile
- Release 0.8.3
- Bump version to 0.8.4
- *(release)* V0.8.5

### 💼 Other

- *(deps)* Update zip requirement from 4 to 5
- *(deps)* Update zip requirement from 5 to 6
- Bump kiddo as 5.1 was yanked
- *(deps)* Update zip requirement from 7 to 8
- *(deps)* Update reqwest requirement from 0.12 to 0.13
- *(deps)* Update itertools requirement from 0.14 to 0.15
## [0.7.0] - 2025-05-11

### 🚀 Features

- Rkyv zero copy deserialization

### 🐛 Bug Fixes

- Clippy, don't use anyhow for example

### 🧪 Testing

- Readme examples

### ⚙️ Miscellaneous Tasks

- *(release)* V0.7.0

### 💼 Other

- *(deps)* Bincode v2.0 and ntex v2.12
## [0.6.6] - 2025-02-13

### 🐛 Bug Fixes

- Manually update demo index
- Update demo dockerfile

### ⚙️ Miscellaneous Tasks

- *(release)* 0.6.6
## [0.6.5] - 2024-12-16

### 🚀 Features

- *(storage)* [**breaking**] Ref #11 introduce trait IndexStorage as first step to custom storages support, now index has metadata

### 🐛 Bug Fixes

- *(service)* Fix ?countries= api filter
- Update ntex deps, clippy fixes
- Use only long args for geosuggest-build-index cli
- Use tree index to geonames id map
- Less allocations on index build and read

### ⚙️ Miscellaneous Tasks

- Github actions
- Use workspace version and authors
- Cleanup Cargo.toml
- Try to solve - Dependabot can't resolve your Rust dependency files
- Move examples under geosuggest-examples to enable dependabot
- Run once workflow
- Try to use actions-rust-lang
- Try don't use --all-features
- *(release)* 0.6.5

### 💼 Other

- Demo
- Use cities5000.zip
- :into_iter_on_ref
- :needless_pass_by_value
- :float-cmp
- :inefficient_to_string
- :explicit_into_iter_loop
