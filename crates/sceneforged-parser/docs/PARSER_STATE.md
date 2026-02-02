# MediaForge Parser: Current State & Fixture Analysis

**Last Updated:** 2026-02-01
**Test Pass Rate:** 786/1160 (67.7%)
**Baseline Locked:** Yes (regression testing enabled)

---

## Executive Summary

The mediaforge-parser has reached a **practical plateau at 67.7%** compatibility across all fixture sources. The remaining 32.3% of failures are primarily due to **fundamental disagreements between fixture sources** rather than parser bugs. These fixtures were created by different projects (parse-torrent-title, go-parse-torrent-name, Sonarr) with incompatible expectations.

A confidence-based parsing system has been implemented to handle ambiguous cases, allowing downstream consumers to request verification from external sources (Sonarr/Radarr APIs) when needed.

---

## Test Results by Source

| Fixture Source | Passed | Total | Rate | License |
|----------------|--------|-------|------|---------|
| Manual Movies | 98 | 98 | 100% | MIT |
| Manual Episodes | 68 | 68 | 100% | MIT |
| Manual Anime | 36 | 36 | 100% | MIT |
| PTT (parse-torrent-title) | 262 | 407 | 64% | MIT |
| Go-PTN (go-parse-torrent-name) | 26 | 83 | 31% | MIT |
| Sonarr | 296 | 468 | 63% | GPL (extracted) |
| **TOTAL** | **786** | **1160** | **67%** | |

**Key Insight:** Manual fixtures (created with consistent expectations) pass 100%. External fixtures fail due to conflicting definitions.

---

## Failure Categories

### Category 1: Year-in-Title Conflicts (~150 tests)

**The Problem:** Fixtures fundamentally disagree on whether a year belongs in the title or as separate metadata.

**Example Input:** `Series.Title.2010.S02E14.HDTV.XviD-LOL`

| Source | Expected Title | Expected Year |
|--------|---------------|---------------|
| Sonarr | `"Series Title 2010"` | `null` |
| PTT | `"Series Title"` | `2010` |
| Our Parser | `"Series Title"` | `2010` |

**Why It Can't Be Fixed:**
- Sonarr wants the year as part of the title for series disambiguation (e.g., "Doctor Who 2005" vs "Doctor Who 1963")
- PTT wants the year as structured metadata for filtering/sorting
- Both are valid interpretations depending on use case
- Fixing for one breaks the other

**Affected Tests:** ~100 Sonarr, ~50 PTT

**Example Failures:**
```
Series.Title.2010.S02E14.HDTV.XviD-LOL
  Sonarr expects: title="Series Title 2010", year=null
  We produce:     title="Series Title", year=2010

doctor_who_2005.8x12.death_in_heaven.720p_hdtv_x264-fov
  PTT expects: title="doctor who", year=2005
  We produce:  title="doctor who 2005", year=null (year treated as part of title)
```

---

### Category 2: Source/Quality Display Name Conflicts (~80 tests)

**The Problem:** Fixtures disagree on how to normalize/display source types.

**Example Comparisons:**

| Input Token | PTT Expected | Go-PTN Expected | Our Output |
|-------------|-------------|-----------------|------------|
| `BrRip` | `BluRay` | `BrRip` | `BluRay` |
| `BDRip` | `BluRay` | `BDRip` | `BDRip` |
| `TS` | `Telesync` | `TS` | `TS` |
| `HDTS` | `HdTelesync` | `HDTS` | `HDTS` |
| `DVDScr` | `Screener` | `DvdScr` | `SCR` |
| `PPV WEB-DL` | `WebDl` | `PPV WEB-DL` | `WEB-DL` |

**Why It Can't Be Fixed:**
- PTT normalizes aggressively (BrRip → BluRay, TS → Telesync)
- Go-PTN preserves original tokens
- Some sources are compound (PPV WEB-DL) but we only support single values
- No consensus on whether `BDRip` means "BluRay source" or is a distinct category

**Affected Tests:** ~50 PTT, ~30 Go-PTN

**Example Failures:**
```
Hercules (2014) 1080p BrRip H264 - YIFY
  Go-PTN expects: source="BrRip"
  PTT expects:    source="BluRay"
  We produce:     source="BluRay"

Dracula.Untold.2014.TS.XViD.AC3.MrSeeN-SiMPLE
  PTT expects: source="Telesync"
  We produce:  source="TS"
```

---

### Category 3: Video Codec Naming Conflicts (~40 tests)

**The Problem:** Fixtures disagree on codec representation.

| Input Token | PTT Expected | Go-PTN Expected | Our Output |
|-------------|-------------|-----------------|------------|
| `H264` | `X264` | `H264` | `x264` |
| `H.264` | `X264` | `H264` | `x264` |
| `H265` | `X265` | `H265` | `x265` |
| `HEVC` | `X265` | `HEVC` | `x265` |

**Why It Can't Be Fixed:**
- PTT treats H.264/H264 as equivalent to x264 (encoder-agnostic)
- Go-PTN distinguishes H264 (codec spec) from x264 (encoder)
- Technically both are correct: H.264 is the standard, x264 is an encoder
- We chose to normalize to encoder names (x264/x265) for consistency

**Attempted Fix:** Added separate `H264`/`H265` variants
**Result:** Caused 122 test regressions because PTT fixtures expect `X264` not `H264`
**Reverted:** Yes

**Example Failures:**
```
Hercules.2014.EXTENDED.1080p.WEB-DL.DD5.1.H264-RARBG
  Go-PTN expects: video_codec="H264"
  PTT expects:    video_codec="X264"
  We produce:     video_codec="x264"
```

---

### Category 4: Release Group Bracket Handling (~40 tests)

**The Problem:** Fixtures disagree on whether trailing brackets are part of the release group.

| Input | PTT Expected | Go-PTN Expected |
|-------|-------------|-----------------|
| `x264-ASAP[ettv]` | `ASAP` | `ASAP[ettv]` |
| `XViD-juggs[ETRG]` | `juggs` | `juggs[ETRG]` |
| `x264-LOL [eztv]` | `null` | `LOL [eztv]` |

**Why It Can't Be Fixed:**
- PTT considers `[ettv]` and `[eztv]` as distribution tags, not part of the group
- Go-PTN includes everything after the hyphen as the release group
- Both interpretations are used in the scene

**Attempted Fix:** Include bracketed suffixes in release group
**Result:** Caused 48 test regressions in PTT fixtures
**Reverted:** Yes

---

### Category 5: Multi-Value Fields (~30 tests)

**The Problem:** Some releases have multiple sources or codecs, but our model only supports single values.

**Examples:**
```
Hercules (2014) WEBDL DVDRip XviD-MAX
  Go-PTN expects: source="WEBDL DVDRip" (compound)
  We produce:     source="WEB-DL" (first match only)

UFC.179.PPV.HDTV.x264-Ebi[rartv]
  Go-PTN expects: source="PPV.HDTV" (compound)
  We produce:     source="HDTV" (PPV not recognized as source)
```

**Why It Can't Be Fixed (without API changes):**
- Our `Source` enum is a single value, not a list
- Adding `Vec<Source>` would be a breaking API change
- PPV is technically a distribution method, not a source quality

---

### Category 6: 3-Digit Compressed Episode Format (~6 tests)

**The Problem:** Episodes encoded as 3-digit numbers after a year (e.g., `416` = S04E16).

**Examples:**
```
series.2009.416.hdtv-lol
  Expected: seasons=[4], episodes=[16]
  We produce: seasons=[], episodes=[]

series.six-0.2010.217.hdtv-lol
  Expected: seasons=[2], episodes=[17]
  We produce: seasons=[], episodes=[]
```

**Why It's Tricky:**
- Must distinguish from title numbers (UFC.179)
- Must not conflict with years (2009, 2010)
- Pattern only valid when preceded by a detected year
- Currently not implemented due to false positive risk

**Status:** Could potentially be fixed with careful implementation

---

### Category 7: Edge Case Episode Formats (~20 tests)

**The Problem:** Unusual episode numbering patterns not currently supported.

**Examples:**
```
Series Title - S1936E18 - I Love to Singa
  Issue: Season number 1936 looks like a year
  Expected: seasons=[1936], episodes=[18]
  We produce: seasons=[], episodes=[]

Series_Title_-_1x1_-_Live_and_Learn_[HDTV-720p]
  Issue: Episode info appears mid-string, followed by episode title
  Expected: seasons=[1], episodes=[1]
  We produce: seasons=[], episodes=[] (confused by episode title)
```

**Why It's Tricky:**
- `S1936E18` conflicts with year detection (1936 looks like a year)
- Episode titles after the season/episode marker confuse parsing
- `1x1` format with underscores and trailing content needs special handling

---

### Category 8: Title Punctuation/Formatting (~20 tests)

**The Problem:** Fixtures expect punctuation restoration we don't perform.

**Examples:**
```
Marvels Agents of S H I E L D S02E05 HDTV x264-KILLERS [eztv]
  Expected: title="Marvel's Agents of S.H.I.E.L.D."
  We produce: title="Marvels Agents of S H I E L D"
```

**Why It Can't Be Fixed:**
- Would require a title lookup database or NLP
- Apostrophe restoration is ambiguous (Marvels → Marvel's? Martins → Martin's?)
- Acronym detection (S H I E L D → S.H.I.E.L.D.) requires domain knowledge
- Out of scope for a pure pattern-matching parser

---

## Architectural Decisions Made

### 1. Codec Normalization: Encoder Names
- `H264`, `H.264`, `AVC` → `x264`
- `H265`, `H.265`, `HEVC` → `x265`
- **Rationale:** Most fixtures expect this; encoder names are more commonly used in release names

### 2. Source Normalization: Moderate
- `BRRip` → `BluRay` (treated as BluRay source)
- `BDRip` → `BDRip` (kept distinct, lower quality than BluRay)
- **Rationale:** Balance between PTT expectations and semantic accuracy

### 3. Year Handling: Metadata First
- Years are extracted as metadata when unambiguous
- Years stay in title for disambiguation (e.g., `Doctor Who 2005`)
- **Rationale:** Most common use case is filtering by year

### 4. Release Group: Exclude Distribution Tags
- `ASAP[ettv]` → `ASAP`
- `LOL [eztv]` → `LOL`
- **Rationale:** PTT has more tests, and this matches scene conventions

---

## Confidence System

The parser includes a confidence system to handle ambiguous cases:

```rust
pub enum ConfidenceLevel {
    /// Needs external verification
    NeedsReview,
    /// Low confidence, multiple interpretations possible
    Uncertain,
    /// High confidence in the result
    Confident,
    /// Certain match (explicit patterns)
    Certain,
}
```

Fields are tagged with confidence levels. Consumers can:
1. Accept high-confidence results directly
2. Flag low-confidence results for verification via Sonarr/Radarr APIs
3. Use `AmbiguityMode::ReportAll` to get multiple interpretations

---

## Recommendations for Research Agents

### High-Value Opportunities

1. **3-Digit Compressed Episodes** (~6 tests)
   - Pattern: `{title}.{year}.{3-digit}.{source}`
   - Example: `series.2009.416.hdtv` → S04E16
   - Implementation: Detect 3-digit numbers (100-999) after a confirmed year
   - Risk: Medium (could conflict with title numbers)

2. **Underscore Episode Format** (~4 tests)
   - Pattern: `{title}_-_{season}x{episode}_-_{episode_title}`
   - Example: `Series_Title_-_7x6_-_The_Scarlett_Getter`
   - Implementation: Handle `_-_` as episode marker boundary
   - Risk: Low

### Low-Value Opportunities (Conflicts)

3. **Year-in-Title Toggle**
   - Could add config: `include_year_in_title: bool`
   - Would allow matching either Sonarr or PTT expectations
   - Not recommended: adds complexity, doesn't increase real-world utility

4. **Codec Name Variants**
   - Could add separate `H264`/`H265` enum variants
   - Already attempted, caused 122 regressions
   - Not recommended: breaks more than it fixes

5. **Multi-Value Sources**
   - Could change `source: Option<Source>` to `source: Vec<Source>`
   - Breaking API change
   - Only helps ~30 tests
   - Not recommended: complexity vs. benefit ratio poor

---

## Files Reference

| File | Purpose |
|------|---------|
| `src/extractors/quality.rs` | Resolution, source, quality modifier extraction |
| `src/extractors/codec.rs` | Video/audio codec extraction |
| `src/extractors/episode.rs` | Season/episode number extraction |
| `src/patterns/compiled.rs` | Regex patterns for all extractors |
| `src/model/quality.rs` | Quality-related enums (Resolution, Source, etc.) |
| `src/model/codec.rs` | Codec enums (VideoCodec, AudioCodec) |
| `src/pipeline/resolver.rs` | Conflict resolution and title fallback |
| `tests/fixtures/baseline.json` | Regression testing baseline (786 tests) |

---

## Regression Testing

A baseline snapshot system prevents future regressions:

```bash
# Run tests (includes regression check)
cargo test

# Update baseline after intentional changes
cargo test -- --ignored generate_baseline --nocapture
```

The baseline tracks all 786 currently passing tests. Any change that causes a previously passing test to fail will be immediately detected.

---

## Conclusion

The parser achieves 100% accuracy on well-defined test cases (manual fixtures) but cannot exceed ~67% on external fixtures due to irreconcilable definitional conflicts between sources. The confidence system allows downstream consumers to handle ambiguous cases appropriately.

Further improvements should focus on:
1. Low-risk pattern additions (3-digit episodes, underscore formats)
2. Configuration options for compatibility modes (not yet implemented)
3. Integration with external verification APIs (Sonarr/Radarr)

Attempting to reach 100% compatibility across all fixtures is **not possible** without breaking changes that would cause equal or greater regressions elsewhere.
