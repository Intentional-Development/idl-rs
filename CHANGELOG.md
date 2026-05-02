## [v0.9.5] - 2026-05-02

- Wave 18 release: Schema v0.1.7 (paginated kind), TypeExpr crate, Discriminator corpus validated


## [0.9.4] - 2025-01-20

### Wave 17 Results
- **TypeExpr DSL:** Prototype design complete, EBNF grammar covers all v0.1.6 kinds, round-trip lossless for type shape. Implementation path: `idl-rs/idl-typeexpr/` crate (W18+).
- **Paginated Validation:** Corpus-2 validated (141 schemas: 112 Stripe cursor-based + 29 firefly-iii page-based). Pagination warrants `kind: "paginated"` in v0.1.7 (W18).
- **Firefly-iii v0.1.6 Extraction:** Re-extracted with array-alias (24 schemas) + union (1 schema). DTO count 251 (24 NEW, not collapses). Conformance 99.6%/76.2% maintained. v0.1.6 extract/emit validated.
- **All W16 unresolved items CLOSED:** TypeExpr designed, pagination validated, firefly-iii extracted.


All notable changes to idl-rs will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.9.3] - 2025-01-03

- **idl-rs:** ArrayAlias + Union variants in DtoKind, nullable field, 16 new tests including backward-compat.

## [0.9.2] - 2026-05-02

### Added (Wave 15)
- **DtoKind enum:** `Object`, `Enum`, `Map`, `Unit` variants with serde discriminator
- **Structured `when` module:** `extensions_when.rs` (220 lines) with `When`, `WhenStructured`, `WhenVar` types
- **8 new validation rules:** dto-kind-valid, dto-object-requires-base, dto-enum-requires-values, dto-enum-no-projection, dto-map-requires-value-type, dto-map-value-type-resolves, dto-map-no-projection, dto-unit-no-fields
- **13 total new tests:** 6 for structured when, 7 for kind discriminator (enum/map/unit/object validation + backward-compat)

### Changed
- Schema compatibility: now validates graph.version against v0.1.5
- DtoDefinition struct: `kind: DtoKind`, `base: Option<String>` (was required), `values`, `value_type`, `key_type`, `nullable` fields added
- Edge `when` parsing: accepts string OR structured object form

### Fixed
- Firefly-iii conformance: 76.2% → 99.6% strict (59 DTOs rescued via kind discriminator)
- Localsend conformance: 66.7% → 100.0% strict (3 DTOs rescued via kind discriminator)

## [0.9.1] - 2026-05-02

### Added (Wave 14)
- Wrapper DTO validation rules: `dto-wrapper-requires-wraps`, `dto-wrapper-wraps-resolves`, `dto-wrapper-no-projection`
- Wrapper DTO OpenAPI emitter logic (property name derivation from wrapped DTO, pluralization support)
- 4 unit tests for wrapper DTO validation

### Changed
- Schema compatibility: now validates graph.version against v0.1.3
- DtoDefinition struct: added `wrapper: bool` and `wraps: Option<String>` fields
- OpenAPI emitter: +27 LOC for wrapper DTO emission

### Fixed
- Wrapper DTO conformance for RealWorld corpus (schemas axis 40% → 100%)

## [0.9.0] - 2026-05-02

### Added (Waves 10-13 Cumulative)
- Wave 13: Design consensus on 4/5 deferred questions (typed_ports + type_compatibility extensions)
- Wave 13: Canonical-DTO gap closure across 4 corpora (n8n 93.8%, firefly-iii 76.2%, localsend 66.7% schema strict conformance)
- Wave 12: DTO extension namespace support (`extensions.dto.definitions[]`)
- Wave 12: Direction C RFC implementation (pick/omit/required/extras validation)
- Wave 12: 10 new DTO validation rules in idl-cli validator
- Wave 12: OpenAPI emitter canonical DTO resolution (#resolves definitions → components/schemas)
- Wave 12: 12 DTO-focused unit tests across validator and emitter
- Wave 11: DTO RFC implementation scaffolding and extension spec validation hooks (typed_ports, type_compatibility)
- Wave 10: OpenAPI emitter components.schemas grouping (fixes realworld P4 blocker)
- Wave 10: 3 emitter tests + 3 drift tests (total: 55 workspace tests)

### Changed
- Schema compatibility: now validates graph.version against v0.1.2
- Workspace test count: 49 → 55 (+6 from W10 emitter/drift fixes)
- Cargo workspace version bumped to 0.9.0 (all member crates synced)
- Conformance schemas: realworld 100% strict (W12), n8n 93.8% strict (W13), firefly-iii 76.2% strict (W13), localsend 66.7% strict (W13)

### Fixed
- Wave 10: OpenAPI emitter component grouping logic (operations now reference shared schemas)
- Wave 10: 5 drift tool edge cases (missing node detection, shifted anchor resolution)

## [0.7.0] - 2026-05-02

### Added
- Wave 10: idl-emitters crate enhancements (OpenAPI grouping, drift JSON export)
- Wave 10: idl-cli drift command 5 polish fixes

### Fixed
- Wave 10: OpenAPI emitter component.schemas generation
- Wave 10: Drift reporting edge cases (missing vs shifted nodes)

## [0.6.0-rc] - 2026-04-30

### Added
- Wave 9: idl interview command implementation
- Wave 8: Anchor validator + rewrite helper
- Wave 8: Schema v0.1.1 support
- Wave 8: Drift (graph + code) command
- Wave 8: idl emit (rust/ts/openapi) commands

### Changed
- CLI restructured into workspace (idl-cli, idl-emitters, idl-validator crates)

---

[0.10.0]: https://github.com/Intentional-Development/idl-rs/compare/v0.6.0-rc...v0.10.0
[0.9.0]: https://github.com/Intentional-Development/idl-rs/compare/v0.6.0-rc...v0.9.0
[0.8.0]: https://github.com/Intentional-Development/idl-rs/compare/v0.6.0-rc...v0.8.0
[0.7.0]: https://github.com/Intentional-Development/idl-rs/compare/v0.6.0-rc...v0.7.0
[0.6.0-rc]: https://github.com/Intentional-Development/idl-rs/releases/tag/v0.6.0-rc
