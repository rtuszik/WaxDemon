# Changelog
All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

- - -
## [v1.9.0](https://github.com/rtuszik/WaxDemon/compare/e50c9248f0d0c1d7f54b8e37c6d4240dc3935a8d..v1.9.0) - 2026-09-16
#### Features
- (**app**) add version footer to the application layout - ([4f48b11](https://github.com/rtuszik/WaxDemon/commit/4f48b11f5a5b8f0a9a9772799cf8c931d9695d98)) - [@rtuszik](https://github.com/rtuszik)
#### Continuous Integration
- cache release versioning tools - ([e50c924](https://github.com/rtuszik/WaxDemon/commit/e50c9248f0d0c1d7f54b8e37c6d4240dc3935a8d)) - [@rtuszik](https://github.com/rtuszik)
#### Refactoring
- (**app**) use utility classes for footer text styles - ([c88e80d](https://github.com/rtuszik/WaxDemon/commit/c88e80d4539f63f60244afdfd0eebf0836b358e1)) - coderabbitai[bot]

- - -

## [v1.8.0](https://github.com/rtuszik/WaxDemon/compare/c8a6a71e7c90dc13ae344ed1aa4ba8bb3550b070..v1.8.0) - 2026-09-16
#### Features
- (**history**) backfill inferred collection history points - ([c8a6a71](https://github.com/rtuszik/WaxDemon/commit/c8a6a71e7c90dc13ae344ed1aa4ba8bb3550b070)) - [@rtuszik](https://github.com/rtuszik)
#### Bug Fixes
- (**sync**) exclude inferred history from sync creation date onward - ([7915f9f](https://github.com/rtuszik/WaxDemon/commit/7915f9f4c329f11890b4c91d61a19f68c4efd1a1)) - [@rtuszik](https://github.com/rtuszik)
- (**sync**) normalize inferred history dates to UTC - ([9d90ee4](https://github.com/rtuszik/WaxDemon/commit/9d90ee4e4e245ec1b5131f1b8ab6f25e2022f3a3)) - [@rtuszik](https://github.com/rtuszik)

- - -

## [v1.7.0](https://github.com/rtuszik/WaxDemon/compare/f7913279243d80bf94a1ba2edfdc967d722fe067..v1.7.0) - 2026-09-15
#### Features
- (**auth**) bootstrap first user as approved admin on fresh installs - ([8481522](https://github.com/rtuszik/WaxDemon/commit/84815226776695f852238135fd2ca8a97eb72bbc)) - [@rtuszik](https://github.com/rtuszik)
#### Documentation
- clarify OAuth keyring Docker mount configuration - ([d52a95f](https://github.com/rtuszik/WaxDemon/commit/d52a95f2da7d6b63f45c12d061362ceb5bef5080)) - [@rtuszik](https://github.com/rtuszik)
- update setup and authentication documentation - ([59742eb](https://github.com/rtuszik/WaxDemon/commit/59742eb62be340e000fa6385ba023ab4b880e001)) - [@rtuszik](https://github.com/rtuszik)
#### Build system
- rename Docker Compose configuration and update documentation - ([6dc09df](https://github.com/rtuszik/WaxDemon/commit/6dc09df476ce68d7de6d7433e9aee375d10279e5)) - [@rtuszik](https://github.com/rtuszik)
#### Refactoring
- (**app**) move sync panel to settings page header - ([99668f2](https://github.com/rtuszik/WaxDemon/commit/99668f24b88910766c510a2420a7d6e1d6f86263)) - [@rtuszik](https://github.com/rtuszik)
#### Style
- remove approval queue description - ([f791327](https://github.com/rtuszik/WaxDemon/commit/f7913279243d80bf94a1ba2edfdc967d722fe067)) - [@rtuszik](https://github.com/rtuszik)

- - -

## [v1.6.0](https://github.com/rtuszik/WaxDemon/compare/f37837c5e2c1910da234d29584b2f90930315443..v1.6.0) - 2026-09-14
#### Features
- (**auth**) add trusted proxy-aware per-identity rate limiting - ([755a8dd](https://github.com/rtuszik/WaxDemon/commit/755a8dde0fb640766a28106de047406b467d7d23)) - [@rtuszik](https://github.com/rtuszik)
- (**auth**) add security limits and configurable session lifetimes - ([d9f294a](https://github.com/rtuszik/WaxDemon/commit/d9f294a84f56ad5660ba1531b620d4dd871dbf0a)) - [@rtuszik](https://github.com/rtuszik)
- (**helm**) disable service account token and set session duration - ([fd415d7](https://github.com/rtuszik/WaxDemon/commit/fd415d7b12dc0c8f29aeb14c30437a2d9fdf2950)) - [@rtuszik](https://github.com/rtuszik)
#### Tests
- cover quota sharing across duplicate forwarding headers - ([4b06880](https://github.com/rtuszik/WaxDemon/commit/4b06880bced5b6614989807bbbb4175adc02c95b)) - [@rtuszik](https://github.com/rtuszik)
#### Miscellaneous Chores
- secure database configuration and remove exposed port - ([f37837c](https://github.com/rtuszik/WaxDemon/commit/f37837c5e2c1910da234d29584b2f90930315443)) - [@rtuszik](https://github.com/rtuszik)

- - -

## [v1.5.1](https://github.com/rtuszik/WaxDemon/compare/fd39d81f4b2411bb481160d9aa4ffa3f5d8ddd4f..v1.5.1) - 2026-09-14
#### Build system
- cache Docker dependencies and update Rust toolchain - ([0ec08a5](https://github.com/rtuszik/WaxDemon/commit/0ec08a51c514872e18a246d7b286417f4cd6b708)) - [@rtuszik](https://github.com/rtuszik)
#### Miscellaneous Chores
- (**deps**) update dependency rust (#41) - ([fd39d81](https://github.com/rtuszik/WaxDemon/commit/fd39d81f4b2411bb481160d9aa4ffa3f5d8ddd4f)) - koalabot-rt[bot], koalabot-rt[bot]
#### Style
- remove condition estimate disclaimer from record details - ([23964f7](https://github.com/rtuszik/WaxDemon/commit/23964f785ad867c94c2745e39546030ea295252a)) - [@rtuszik](https://github.com/rtuszik)

- - -

## [v1.5.0](https://github.com/rtuszik/WaxDemon/compare/157af6aa2800b7e48946bc80120d9aedac579306..v1.5.0) - 2026-09-13
#### Features
- (**dev**) add local development task with environment config - ([78b40da](https://github.com/rtuszik/WaxDemon/commit/78b40da0b1d124cf042113f892fd6b05bccaf429)) - [@rtuszik](https://github.com/rtuszik)
#### Refactoring
- remove chart currency selection control - ([40f9eff](https://github.com/rtuszik/WaxDemon/commit/40f9eff8f323cc241b6fd0cf1a04ced85171d859)) - [@rtuszik](https://github.com/rtuszik)
#### Miscellaneous Chores
- (**deps**) update azure/setup-helm digest to 9bc31f4 - ([b6a482b](https://github.com/rtuszik/WaxDemon/commit/b6a482b08bf9c3e6c1889cf6fbc860f4968ac690)) - koalabot-rt[bot]
- (**deps**) update docker/login-action digest to dbcb813 - ([6d52f35](https://github.com/rtuszik/WaxDemon/commit/6d52f35d9508d31f601819f0ad95efa7d7a3f2a9)) - koalabot-rt[bot]
- (**deps**) update docker/metadata-action digest to dc80280 - ([2aad021](https://github.com/rtuszik/WaxDemon/commit/2aad0213859ae5df34c90ec1ded89fce3930987c)) - koalabot-rt[bot]
- (**deps**) update docker/build-push-action digest to 53b7df9 - ([c0163e1](https://github.com/rtuszik/WaxDemon/commit/c0163e151b518659564d00ff5e8c71c6df2970bf)) - koalabot-rt[bot]
- (**deps**) update docker/setup-buildx-action digest to 37fe631 - ([b1974b2](https://github.com/rtuszik/WaxDemon/commit/b1974b2c0e0cedde3bea032b3cb6edb777fc2e19)) - koalabot-rt[bot]
- (**deps**) update rust crate async-trait to v0.1.92 - ([e5972f4](https://github.com/rtuszik/WaxDemon/commit/e5972f463bf080480e3258a3a9cc7702cb8b67b2)) - koalabot-rt[bot]
- (**deps**) update jdx/mise-action digest to c2a8761 - ([ed50c08](https://github.com/rtuszik/WaxDemon/commit/ed50c081c1eae4a31ea55f3445c7268e9d8212c8)) - koalabot-rt[bot]
- (**deps**) update swatinem/rust-cache digest to 6323deb - ([8d95bf9](https://github.com/rtuszik/WaxDemon/commit/8d95bf9b2db4c530758b18380be4ffb2db6c2a30)) - koalabot-rt[bot]
- (**deps**) update rust crate thiserror to v2.0.20 - ([f10fa72](https://github.com/rtuszik/WaxDemon/commit/f10fa727c2ec41d250899bd9315a7d5cb1950912)) - koalabot-rt[bot]
- (**deps**) update rust crate leptos_axum to v0.8.10 - ([5e05ca6](https://github.com/rtuszik/WaxDemon/commit/5e05ca6633965933c08f88e0ba0dcecf883c9341)) - koalabot-rt[bot]
- (**deps**) update rust crate leptos_router to v0.8.15 - ([1ff84fa](https://github.com/rtuszik/WaxDemon/commit/1ff84faff51822af997337e8f8f70208255c0ab1)) - koalabot-rt[bot]
- (**deps**) update rust crate reqwest to v0.13.4 - ([6bfe937](https://github.com/rtuszik/WaxDemon/commit/6bfe937f8245777896c91196971d0372489195a4)) - koalabot-rt[bot]
- (**deps**) update dependency helm to v3.21.4 - ([cb9e205](https://github.com/rtuszik/WaxDemon/commit/cb9e205293a7a16af6c208fd49d828f4e6dd3889)) - koalabot-rt[bot]
- (**deps**) update dependency jdx/mise to v2026.9.1 - ([89b58ec](https://github.com/rtuszik/WaxDemon/commit/89b58ec648c4365243356cccf38910e5f85e9894)) - koalabot-rt[bot]
- (**deps**) update rust crate tower-http to 0.7 - ([d3ca7b7](https://github.com/rtuszik/WaxDemon/commit/d3ca7b719b485444515a18fb1dd53cc7e8fed33f)) - koalabot-rt[bot]
- (**deps**) update dependency oxfmt to v0.66.0 - ([5958854](https://github.com/rtuszik/WaxDemon/commit/5958854eb864a7db64caa30769c408af090045e8)) - koalabot-rt[bot]
- (**deps**) update actions/checkout action to v7 - ([0ba5ead](https://github.com/rtuszik/WaxDemon/commit/0ba5eade8130449db22ee67a23ef69f656eff4db)) - koalabot-rt[bot]
#### Style
- remove history chart currency note - ([157af6a](https://github.com/rtuszik/WaxDemon/commit/157af6aa2800b7e48946bc80120d9aedac579306)) - [@rtuszik](https://github.com/rtuszik)

- - -

## [v1.4.1](https://github.com/rtuszik/WaxDemon/compare/3a1cd8010c1b4154e0ece33ba5787ba6a0adb3c1..v1.4.1) - 2026-09-13
#### Bug Fixes
- (**hooks**) limit source checks to pre-commit - ([3a1cd80](https://github.com/rtuszik/WaxDemon/commit/3a1cd8010c1b4154e0ece33ba5787ba6a0adb3c1)) - [@rtuszik](https://github.com/rtuszik)
#### Tests
- simplify assertions and await redirects - ([d6eff85](https://github.com/rtuszik/WaxDemon/commit/d6eff85f2212170801ea3af8419751ca0ea479c0)) - [@rtuszik](https://github.com/rtuszik)
#### Style
- remove uppercase text transformations - ([8b4d3a0](https://github.com/rtuszik/WaxDemon/commit/8b4d3a0a5e03d99b11c3ab233d413ae98cec1b92)) - [@rtuszik](https://github.com/rtuszik)

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
