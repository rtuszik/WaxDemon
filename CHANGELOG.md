# Changelog
All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

- - -
## [v1.4.0](https://github.com/rtuszik/WaxDemon/compare/6c720d0e48839e50251d2de26bbcb8458fbf7a7f..v1.4.0) - 2026-09-13
#### Features
- (**auth**) group collection formats into normalized categories - ([32d9b60](https://github.com/rtuszik/WaxDemon/commit/32d9b60d939ed1dd8fb4a066b71bd2fddb26cc79)) - [@rtuszik](https://github.com/rtuszik)
- (**library**) enable grid view by default - ([6c720d0](https://github.com/rtuszik/WaxDemon/commit/6c720d0e48839e50251d2de26bbcb8458fbf7a7f)) - [@rtuszik](https://github.com/rtuszik)

- - -

## [v1.3.0](https://github.com/rtuszik/WaxDemon/compare/40528a62aafee787fc2ba839cc6cf96a5f37cd54..v1.3.0) - 2026-09-13
#### Features
- (**db**) clean up legacy tables after import and migration - ([f6953c2](https://github.com/rtuszik/WaxDemon/commit/f6953c23f0a7edffaf077aec447192dd49f8aa0b)) - [@rtuszik](https://github.com/rtuszik)
- (**library**) add decade filtering and simplify dashboard history - ([a538afe](https://github.com/rtuszik/WaxDemon/commit/a538afeda4490ee58ff00b379e694b370db7937c)) - [@rtuszik](https://github.com/rtuszik)
#### Documentation
- clarify documentation change guidelines - ([80ae281](https://github.com/rtuszik/WaxDemon/commit/80ae2810ba268223cae93bc914e097149a7dd49c)) - [@rtuszik](https://github.com/rtuszik)
#### Style
- Align ranked record amounts to the right - ([c7c6228](https://github.com/rtuszik/WaxDemon/commit/c7c62287dcbe7492343fb250a42d9caf75c276f5)) - [@rtuszik](https://github.com/rtuszik)
- format chart configuration and Rust analyzer settings - ([40528a6](https://github.com/rtuszik/WaxDemon/commit/40528a62aafee787fc2ba839cc6cf96a5f37cd54)) - [@rtuszik](https://github.com/rtuszik)

- - -

## [v1.2.0](https://github.com/rtuszik/WaxDemon/compare/2a48b16790e536b39dbcc8d44747831e918579f6..v1.2.0) - 2026-09-13
#### Features
- (**sync**) add currency validation and sync warnings - ([ed8b687](https://github.com/rtuszik/WaxDemon/commit/ed8b687e059b930f5be8b3592c76fdbb6f3ef77c)) - [@rtuszik](https://github.com/rtuszik)
#### Bug Fixes
- (**auth**) show only latest valued dashboard snapshot - ([34359cd](https://github.com/rtuszik/WaxDemon/commit/34359cd235aed22dedb2b961fde11b0c10454431)) - [@rtuszik](https://github.com/rtuszik)
- (**sync**) retain cached prices on invalid currency responses - ([3156c50](https://github.com/rtuszik/WaxDemon/commit/3156c50e75290f22db057262bed16c2094a2b57e)) - [@rtuszik](https://github.com/rtuszik)
#### Continuous Integration
- streamline release and Rust workflows - ([02289fc](https://github.com/rtuszik/WaxDemon/commit/02289fc3b05b8b9fffb4c1a0b07cff63439aad57)) - [@rtuszik](https://github.com/rtuszik)
- add workflow for commit validation - ([2a48b16](https://github.com/rtuszik/WaxDemon/commit/2a48b16790e536b39dbcc8d44747831e918579f6)) - [@rtuszik](https://github.com/rtuszik)
#### Style
- format Leptos views with leptosfmt - ([5f161b0](https://github.com/rtuszik/WaxDemon/commit/5f161b0fb3d2035b4337e830465b66414d61a9ae)) - [@rtuszik](https://github.com/rtuszik)

- - -

## [v1.1.0](https://github.com/rtuszik/WaxDemon/compare/9b5264b0b1bb1ec5a7794385363672002405ff13..v1.1.0) - 2026-09-12
#### Features
- (**dashboard**) paginate history and compress API responses - ([690e439](https://github.com/rtuszik/WaxDemon/commit/690e439d8a0e3c70e01a7130212108c95c42cb9e)) - [@rtuszik](https://github.com/rtuszik)
#### Continuous Integration
- remove Rust tools from cocogitto validation workflow - ([23a7704](https://github.com/rtuszik/WaxDemon/commit/23a770461d84a8f04ac6930290bb8db7f47161b6)) - [@rtuszik](https://github.com/rtuszik)
#### Miscellaneous Chores
- (**changelog**) configure GitHub author links in changelog generation - ([9b5264b](https://github.com/rtuszik/WaxDemon/commit/9b5264b0b1bb1ec5a7794385363672002405ff13)) - [@rtuszik](https://github.com/rtuszik)
#### Style
- (**ui**) align sync status beneath sync details - ([948d2c2](https://github.com/rtuszik/WaxDemon/commit/948d2c225c7ff86cf836e4f3960f91a9b9d90ce3)) - [@rtuszik](https://github.com/rtuszik)

- - -

## v1.0.0 - 2026-09-12
#### Features
- (**auth**) import legacy data on first owner OAuth login - (a330747) - [@rtuszik](https://github.com/rtuszik)
- (**auth**) add Discogs OAuth authentication and session management - (cbff80e) - [@rtuszik](https://github.com/rtuszik)
- (**db**) add multi-user legacy data migration workflow - (d865120) - [@rtuszik](https://github.com/rtuszik)
- (**release**) support manual version bumps and custom release notes [skip ci] - (ba85bac) - [@rtuszik](https://github.com/rtuszik)
- (**ui**) add library browser and ECharts dashboard - (41bb6c7) - [@rtuszik](https://github.com/rtuszik)
- <span style="background-color: #d73a49; color: white; padding: 2px 6px; border-radius: 3px; font-weight: bold; font-size: 0.85em;">BREAKING</span>configure OAuth secrets and production frontend build - (de0dfe7) - [@rtuszik](https://github.com/rtuszik)
- add multi-user OAuth collection sync and job processing - (f89d465) - [@rtuszik](https://github.com/rtuszik)
- add secure Discogs OAuth credential storage - (74b15e2) - [@rtuszik](https://github.com/rtuszik)
- add rust-analyzer tool configuration - (4a0c821) - [@rtuszik](https://github.com/rtuszik)
#### Documentation
- update OAuth setup and upgrade instructions - (c83b91b) - [@rtuszik](https://github.com/rtuszik)
#### Continuous Integration
- add WASM, browser and deployment checks - (b6f0870) - [@rtuszik](https://github.com/rtuszik)
#### Style
- (**dashboard**) adjust chart time range spacing - (3f4557a) - [@rtuszik](https://github.com/rtuszik)

- - -

## v0.3.1 - 2026-08-30
#### Bug Fixes
- (**changelog**) restore cocogitto separators and exclude from oxfmt - (4fe14e3) - [@rtuszik](https://github.com/rtuszik)
- (**docker**) correct PostgreSQL data volume mount path - (fa7f388) - [@rtuszik](https://github.com/rtuszik)
#### Documentation
- remove outdated release workflow documentation - (8cc8939) - [@rtuszik](https://github.com/rtuszik)
#### Build system
- automate Cargo version updates during releases - (e52949b) - [@rtuszik](https://github.com/rtuszik)
#### Continuous Integration
- add pre-commit hooks for code quality checks - (7fcaaa4) - [@rtuszik](https://github.com/rtuszik)
#### Refactoring
- (**core**) simplify empty format handling - (2864127) - [@rtuszik](https://github.com/rtuszik)
#### Miscellaneous Chores
- harden containers and release workflows - (68f3c27) - [@rtuszik](https://github.com/rtuszik)
- add opengrep and zizmor tooling with oxfmt task - (c7c6c41) - [@rtuszik](https://github.com/rtuszik)

- - -

## v0.3.0 - 2026-08-24
#### Features
- (**dashboard**) add selectable chart history ranges - (e861f56) - [@rtuszik](https://github.com/rtuszik)
#### Tests
- update dashboard stats empty history assertion - (68d7cd8) - [@rtuszik](https://github.com/rtuszik)
#### Continuous Integration
- update release workflow and tag push handling - (21e6b5a) - [@rtuszik](https://github.com/rtuszik)
- manage Cocogitto with mise in release workflow - (da7f499) - [@rtuszik](https://github.com/rtuszik)
- validate SemVer output in release workflow - (08ad57f) - [@rtuszik](https://github.com/rtuszik)
- set package publishing workflow permissions - (9ce14b4) - [@rtuszik](https://github.com/rtuszik)
- automate releases and publishing via Conventional Commits - (da1e5d0) - [@rtuszik](https://github.com/rtuszik)
#### Miscellaneous Chores
- (**version**) v0.3.0 [skip ci] - (ac12b4f) - github-actions[bot]
- add AGENTS.md symlink to CLAUDE.md - (b7588dd) - [@rtuszik](https://github.com/rtuszik)

- - -

Changelog generated by [cocogitto](https://github.com/cocogitto/cocogitto).
