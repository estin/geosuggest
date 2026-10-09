# Migration notes

This file records migrations between versions that break files or code. Each section names the versions it covers and lists the steps that restore a working setup.

## Version reference

| Code version | Index format version | Note |
|---|---|---|
| 0.8.5 and before | No version field | Last code that reads old index files |
| memory-usage branch | 1 | First code that stamps and checks the format version |

The index format version lives in `geosuggest_core::index::INDEX_FORMAT_VERSION`. Storage dump stamps every index with the current value. Storage load rejects files with a different value. The crate version and the index format version move separately. The table above maps them so you can tell which files load where.

## Migrate from 0.8.5 to the memory-usage branch

The memory-usage branch changes the index format and the public API. Old index files do not load. Some Rust types changed shape.

The migration has three parts: rebuild the index, update code that touches the changed types, and check result order in tests.

### 1. Rebuild the index

Storage load reads the version first and rejects files with a different version.

If the code loads an old index file, it returns this error:

```
index format version 0 is not supported (code expects 1); rebuild the index and retry
```

#### 1.1 Rebuild steps

1. Update the code to the memory-usage branch.
2. Delete the old index file. The default file path depends on your configuration.
3. Build a new index from the source files with `IndexUpdater` or the `geosuggest-build-index` tool.
4. If the program starts with a cached index file, restart it after step 3.

#### 1.2 Automatic detection

If the program uses `IndexUpdater::has_updates`, it detects the version change and reports that a rebuild is required. No code change is needed for that path.

The read-metadata call returns no metadata for old files instead of an error. If the code checks metadata before load, it treats the old file as absent and rebuilds.

### 2. Update Entry country id

The type of `Entry::country_id` changed.

| Before | After |
|---|---|
| `pub country_id: Option<u32>` | `pub country_id: u32` |

The value `NO_COUNTRY_ID` (`u32::MAX`) marks an entry with no known country. The constant lives in `geosuggest_core::index`.

#### 2.1 Update steps

1. Replace each `None` check on `country_id` with a comparison against `NO_COUNTRY_ID`.
2. Replace each `Some(id)` unwrap with the plain `u32` value.

Example:

```rust
use geosuggest_core::index::NO_COUNTRY_ID;

// Before
if let Some(id) = entry.country_id { /* use id */ }

// After
if entry.country_id != NO_COUNTRY_ID { /* use entry.country_id */ }
```

### 3. Update CitiesRecord field reads

`CitiesRecord` no longer stores country, division, timezone, and name data in each record. It stores index numbers into shared tables in `IndexData`. The new accessor methods on `Engine` read the shared tables for you.

| Removed field | Replacement |
|---|---|
| `country: Option<Country>` | `engine.city_country(city)` returns `Option<&ArchivedCountry>` |
| `admin_division: Option<AdminDivision>` | `engine.city_admin1(city)` returns `Option<&ArchivedAdminDivision>` |
| `admin2_division: Option<AdminDivision>` | `engine.city_admin2(city)` returns `Option<&ArchivedAdminDivision>` |
| `timezone: String` | `engine.city_timezone(city)` returns `&str` |
| `country_names` | `engine.country_names(&code)` with the country code |
| `admin1_names` | `engine.admin1_names(id)` with the division geoname id |
| `admin2_names` | `engine.admin2_names(id)` with the division geoname id |

#### 3.1 Update steps

1. Find each read of a removed field on `ArchivedCitiesRecord`.
2. Replace it with the matching accessor from the table above.
3. Keep the `Engine` value alive while you use the returned reference. The reference points into the engine data.
4. Treat the `NO_TABLE_INDEX` case through the accessor return value. A missing value returns `None`.

Example:

```rust
// Before
let country = city.country.as_ref();
let timezone = &city.timezone;

// After
let country = engine.city_country(city);
let timezone = engine.city_timezone(city);
```

### 4. Update IndexData map use

The type of `IndexData::tree_index_to_geonameid` changed.

| Before | After |
|---|---|
| `HashMap<usize, u32>` | `Vec<u32>` |

Index position in the vector is the tree index.

#### 4.1 Update steps

1. Replace map lookups with vector indexing by position.
2. Replace map iteration with vector iteration. The vector order is the tree index order.

The new shared tables (`countries`, `admin1_divisions`, `admin2_divisions`, `timezones`) are read through the `Engine` accessors in section 3. You do not need to read them directly.

### 5. Check suggest result order in tests

The suggest ranking keeps the same order rule: higher score first, then higher population. The order between exact ties changed. An exact tie means two cities with the same score and the same population.

#### 5.1 Update steps

1. Run the test suite.
2. If a test that asserts a fixed result order fails, check that the failure is an exact tie.
3. If it is an exact tie, update the test to accept the new order.

No production code change is needed for this item.

### 6. Full checklist

1. Rebuild the index from source files.
2. Update `Entry::country_id` checks to use `NO_COUNTRY_ID`.
3. Update `CitiesRecord` field reads to use the `Engine` accessors.
4. Update `tree_index_to_geonameid` use to vector indexing.
5. Run the test suite and fix exact-tie order assertions.
