# Benchmark Results

Run on: macOS Darwin 25.2.0
Rust: stable
Profile: release (optimized)

## Parser Throughput (`sf-parser`)

| Input | Time |
|---|---|
| simple_movie (`The.Matrix.1999.1080p.BluRay.x264-GROUP`) | 265 ns |
| 4k_hdr (UHD BluRay Remux HDR DV TrueHD Atmos HEVC) | 371 ns |
| tv_episode (`Breaking.Bad.S01E01.720p.WEB-DL...`) | 311 ns |
| multi_episode (`Show.S01E01E02.1080p...`) | 286 ns |
| long_complex (LotR Extended 2160p UHD x265 HDR10 DTS-HD) | 597 ns |

Scales ~linearly with token count. All well under 1 µs.

## Sendfile classify_peek

| Request Type | Time |
|---|---|
| HLS segment (`/api/stream/{id}/segment_5.m4s`) | 70 ns |
| Direct stream (`/api/stream/{id}/direct`) | 62 ns |
| Jellyfin stream (`/Videos/{id}/stream`) | 74 ns |
| Non-match GET (`/api/items`) | 24 ns |
| POST early exit (`/api/auth/login`) | 11 ns |

Critical hot path (runs on every TCP connection). All sub-100ns.
POST exits in 11ns by checking method before parsing path.

## Segment Map Computation (`sf-media`)

| Duration | Keyframes | Time |
|---|---|---|
| 5 minutes | 75 | 304 ns |
| 30 minutes | 450 | 1.0 µs |
| 2 hours | 1,800 | 3.7 µs |

Perfect O(n) linear scaling. Even 2-hour movies compute in under 4 µs.

## DTO Serialization (Jellyfin)

| Operation | Time |
|---|---|
| item_to_dto (DB Item → BaseItemDto) | 143 ns |
| serde_json::to_string(BaseItemDto) | 283 ns |
| Serialize 50-item ItemsResult | 10.7 µs (~214 ns/item) |

Very fast. Serialization is not a bottleneck.

## DB Item Queries (1,000-item dataset)

| Query | Time |
|---|---|
| get_item_by_id (PK lookup) | 5.6 µs |
| list_items_by_library (limit=50) | 135 µs |
| list_children_ordered (20 episodes) | 24.6 µs |
| count_items_by_library | 15.8 µs |

All fast. PK lookups under 6 µs.

## FTS5 / LIKE Search

### After optimization (subquery-driven FTS)

| Dataset | fts_prefix | fts_library_filter | fts_kind_filter | like_fallback |
|---|---|---|---|---|
| 100 items | 89 µs | 92 µs | 96 µs | 88 µs |
| 1,000 items | 497 µs | 496 µs | 507 µs | 500 µs |
| 5,000 items | 2.4 ms | 2.4 ms | 2.6 ms | 2.4 ms |

### Before optimization (JOIN-driven FTS — pathological query plan)

| Dataset | fts_library_filter (before) | fts_library_filter (after) | Speedup |
|---|---|---|---|
| 100 items | 2.8 ms | 92 µs | **30x** |
| 1,000 items | 80 ms | 496 µs | **161x** |
| 5,000 items | 1.5 s | 2.4 ms | **625x** |

The JOIN caused SQLite to drive from the `items` table (via the `library_id`
index), probing FTS per-row — O(n) FTS lookups. Switching to a subquery
forces FTS to be the driving table, producing rowids that are then filtered
by the outer WHERE clause.
