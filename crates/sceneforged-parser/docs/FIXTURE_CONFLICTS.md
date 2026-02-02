# Fixture Conflict Analysis

This document provides detailed analysis of conflicts between fixture sources for research purposes.

---

## Source Definitions

### PTT (parse-torrent-title)
- **Origin:** Python library for parsing torrent names
- **Philosophy:** Normalize everything to canonical forms
- **License:** MIT
- **Fixture Count:** 407 tests

### Go-PTN (go-parse-torrent-name)
- **Origin:** Go port of parse-torrent-name
- **Philosophy:** Preserve original tokens where possible
- **License:** MIT
- **Fixture Count:** 83 tests

### Sonarr
- **Origin:** TV series management application
- **Philosophy:** Optimize for library matching (years in titles for disambiguation)
- **License:** GPL (fixtures extracted)
- **Fixture Count:** 468 tests

---

## Conflict Type 1: Same Input, Different Expected Output

These inputs appear in multiple fixture sources with contradictory expectations.

### Example 1: BrRip Source
```json
// Input
"Hercules (2014) 1080p BrRip H264 - YIFY"

// PTT expects:
{
  "source": "BluRay",
  "video_codec": "X264"
}

// Go-PTN expects:
{
  "source": "BrRip",
  "video_codec": "H264"
}
```

**Analysis:**
- PTT normalizes `BrRip` → `BluRay` (same source, different rip method)
- Go-PTN preserves `BrRip` as distinct
- PTT normalizes `H264` → `X264` (codec → encoder)
- Go-PTN preserves `H264` as the codec specification

**Winner:** Cannot determine - both are valid interpretations

---

### Example 2: Release Group Brackets
```json
// Input
"The Walking Dead S05E03 720p HDTV x264-ASAP[ettv]"

// PTT expects:
{
  "release_group": "ASAP"
}

// Go-PTN expects:
{
  "release_group": "ASAP[ettv]"
}
```

**Analysis:**
- PTT treats `[ettv]` as distribution tag (not part of group)
- Go-PTN includes everything after hyphen as group name

**Winner:** PTT is more semantically correct (ASAP is the release group, ettv is the distribution site)

---

### Example 3: Year Placement
```json
// Input
"Series.Title.2010.S02E14.HDTV.XviD-LOL"

// Sonarr expects:
{
  "title": "Series Title 2010",
  "year": null
}

// PTT expects:
{
  "title": "Series Title",
  "year": 2010
}
```

**Analysis:**
- Sonarr keeps year in title for disambiguation (multiple series with same name, different years)
- PTT extracts year as metadata for filtering

**Winner:** Depends on use case
- For library matching: Sonarr is correct
- For metadata extraction: PTT is correct

---

## Conflict Type 2: Normalization Disagreements

### Source Type Normalization

| Input Token | PTT | Go-PTN | Our Choice | Notes |
|-------------|-----|--------|------------|-------|
| `BluRay` | `BluRay` | `BluRay` | `BluRay` | Agreement |
| `BrRip` | `BluRay` | `BrRip` | `BluRay` | PTT normalizes |
| `BDRip` | `BluRay` | `BDRip` | `BDRip` | We keep distinct |
| `WEB-DL` | `WebDl` | `WEB-DL` | `WEB-DL` | Case difference |
| `WEBDL` | `WebDl` | `WEBDL` | `WEB-DL` | We normalize |
| `HDTV` | `Hdtv` | `HDTV` | `HDTV` | Case difference |
| `TS` | `Telesync` | `TS` | `TS` | PTT expands |
| `HDTS` | `HdTelesync` | `HDTS` | `HDTS` | PTT expands |
| `DVDScr` | `Screener` | `DvdScr` | `SCR` | All different |
| `R5` | `R5` | `R5` | `R5` | Agreement |
| `PPV` | (ignored) | `PPV` | (ignored) | Distribution, not source |

### Video Codec Normalization

| Input Token | PTT | Go-PTN | Our Choice | Notes |
|-------------|-----|--------|------------|-------|
| `x264` | `X264` | `x264` | `x264` | Case only |
| `X264` | `X264` | `x264` | `x264` | Case only |
| `H264` | `X264` | `H264` | `x264` | PTT normalizes |
| `H.264` | `X264` | `H264` | `x264` | PTT normalizes |
| `AVC` | `X264` | `AVC` | `x264` | All normalize |
| `x265` | `X265` | `x265` | `x265` | Case only |
| `H265` | `X265` | `H265` | `x265` | PTT normalizes |
| `HEVC` | `X265` | `HEVC` | `x265` | PTT normalizes |
| `XviD` | `XVID` | `XviD` | `XviD` | Case variance |

---

## Conflict Type 3: Multi-Value vs Single-Value

Some inputs contain multiple values for a field. Fixtures disagree on handling.

### Example: Compound Source
```json
// Input
"Hercules (2014) WEBDL DVDRip XviD-MAX"

// Go-PTN expects:
{
  "source": "WEBDL DVDRip"  // compound string
}

// PTT expects:
{
  "source": "WebDl"  // first/primary only
}
```

**Our Behavior:** Take first valid match (`WEB-DL`)

**To Support Go-PTN:** Would need `Vec<Source>` or `String` type

---

### Example: PPV + Source
```json
// Input
"UFC.179.PPV.HDTV.x264-Ebi[rartv]"

// Go-PTN expects:
{
  "source": "PPV.HDTV"  // compound
}

// PTT expects:
{
  "source": "Hdtv"  // PPV ignored
}
```

**Our Behavior:** Extract `HDTV` only, `PPV` not a source type

**Analysis:** PPV is a distribution method, not a quality source. Go-PTN's compound approach is unusual.

---

## Conflict Type 4: Title Boundary Detection

### Example: Year as Title Terminator vs Title Component
```json
// Input
"doctor_who_2005.8x12.death_in_heaven.720p_hdtv_x264-fov"

// PTT expects:
{
  "title": "doctor who",
  "year": 2005,
  "seasons": [8],
  "episodes": [12]
}

// Sonarr likely expects:
{
  "title": "doctor who 2005",
  "year": null,
  "seasons": [8],
  "episodes": [12]
}
```

**Our Behavior:** Include year in title when followed by episode marker

**Analysis:** "Doctor Who 2005" is a common disambiguation from "Doctor Who 1963"

---

### Example: Numbers in Title
```json
// Input
"UFC.179.PPV.HDTV.x264-Ebi[rartv]"

// Expected:
{
  "title": "UFC 179"  // number is part of title
}
```

**Our Behavior:** Correctly preserves `179` in title

**Mechanism:** Title number detection checks if number is followed by known metadata tokens

---

## Unimplementable Expectations

### Title Punctuation Restoration
```json
// Input
"Marvels Agents of S H I E L D S02E05 HDTV x264-KILLERS [eztv]"

// PTT expects:
{
  "title": "Marvel's Agents of S.H.I.E.L.D."
}

// We produce:
{
  "title": "Marvels Agents of S H I E L D"
}
```

**Why Unimplementable:**
1. Requires knowledge that "Marvels" → "Marvel's"
2. Requires knowledge that "S H I E L D" → "S.H.I.E.L.D."
3. Would need NLP or title database lookup
4. Ambiguous: "Martins" → "Martin's"? "Martins"? Context-dependent

---

### Episode Title Extraction
```json
// Input
"Series_Title_-_7x6_-_The_Scarlett_Getter_[SDTV]"

// Sonarr expects:
{
  "title": "Series Title!",
  "seasons": [7],
  "episodes": [6]
  // episode_title: "The Scarlett Getter" (not in our model)
}

// We produce:
{
  "title": "Series Title! - 7x6 - The Scarlett Getter",
  "seasons": [],
  "episodes": []
}
```

**Issues:**
1. `!` in title not in input (requires database lookup)
2. Episode title detection would need different architecture
3. `_-_` pattern not currently recognized as separator

---

## Implementable Improvements

### 1. Three-Digit Compressed Episodes

**Pattern:** `{title}.{year}.{3-digit}.{source}`
**Detection:** 3-digit number (100-999) immediately after a detected year

```json
// Input
"series.2009.416.hdtv-lol"

// Expected:
{
  "title": "series",
  "year": 2009,
  "seasons": [4],
  "episodes": [16]
}

// Current:
{
  "title": "series",
  "year": 2009,
  "seasons": [],
  "episodes": []
}
```

**Implementation Notes:**
- Only trigger when year is already detected
- Parse as: first digit = season, remaining = episode
- E.g., `416` → season 4, episode 16
- E.g., `217` → season 2, episode 17

---

### 2. Underscore Episode Separator

**Pattern:** `_-_{season}x{episode}_-_`

```json
// Input
"Series_Title_-_7x6_-_The_Scarlett_Getter_[SDTV]"

// Expected:
{
  "seasons": [7],
  "episodes": [6]
}
```

**Implementation Notes:**
- Detect `_-_` as episode boundary marker
- Extract `{n}x{n}` pattern between markers
- Stop title extraction at first `_-_`

---

### 3. High Season Numbers (Year-like)

**Pattern:** `S{4-digit}E{n}` where 4-digit looks like year

```json
// Input
"Series Title - S1936E18 - I Love to Singa"

// Expected:
{
  "seasons": [1936],
  "episodes": [18]
}

// Current:
{
  "seasons": [],
  "episodes": []
}
```

**Implementation Notes:**
- Currently skipped because 1936 triggers year detection
- Could add special case: if `S{year}E{n}` pattern, treat as season not year
- Risk: Could misparse actual year references

---

## Fixture Source Quality Assessment

| Source | Consistency | Semantic Accuracy | Real-World Relevance |
|--------|-------------|-------------------|---------------------|
| Manual | High | High | High |
| PTT | High | Medium | High |
| Go-PTN | Medium | Medium | Medium |
| Sonarr | Medium | High | High |

**Notes:**
- **PTT:** Consistent within itself but aggressive normalization loses information
- **Go-PTN:** Some unusual expectations (compound sources, brackets in groups)
- **Sonarr:** Optimized for library matching, not general metadata extraction
- **Manual:** Created with clear, consistent rules

---

## Research Recommendations

1. **Do not attempt to reach 100%** - Fixtures are fundamentally incompatible

2. **Focus on low-risk improvements:**
   - 3-digit compressed episodes
   - Underscore separator handling
   - High season number support

3. **Consider configuration modes** for different use cases:
   - `CompatibilityMode::Sonarr` - Years in titles
   - `CompatibilityMode::PTT` - Years as metadata
   - `CompatibilityMode::Default` - Current behavior

4. **Use confidence system** to flag ambiguous parses for external verification

5. **Document breaking expectations** - Some fixtures are simply wrong or context-dependent
