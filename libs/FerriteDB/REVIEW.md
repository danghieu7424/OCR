# FerriteDB repository status

The workspace has seven crates. `ferritedb-core` is the low-level database
package; `ferritedb` is the application-facing facade. Playback-history
schema/migration/upsert is an example built on generic core APIs. WAL metrics,
checkpoint policy, and database/WAL backup/restore remain generic APIs.
No page-size, freelist, overflow-page, or SQL-generalization changes were made.

Windows/MSVC gates pass: formatting, 88 unit/integration tests plus one
doctest, strict Clippy, and `cargo audit` (93 dependencies). Android armv7 and
arm64 workspace cross-checks pass; all 30 pager tests, including the
cross-process reader/checkpoint test, also pass on the SM-J250F shell. The
playback-history example previously persisted and reopened its database on
that device. The 5,000-row WAL-open median improved from 21.598 ms to
13.135 ms after table-driven, fused CRC32C processing. See
[`docs/reviews/2026-11-17-android-device-evaluate.md`](docs/reviews/2026-11-17-android-device-evaluate.md)
for exact commands and measurements.

The Android linker reports `DT_FLAGS_1=0x8000001` on ARMv7 test executables.
`llvm-readelf` confirms `NOW|PIE` and `BIND_NOW`; these hardening flags were
retained. Test execution succeeds despite the warning.

Android validation is limited to this device and its shell-accessible temp
directory; it does not verify Android app sandboxing, and no WBF package is
installed on the test device. The Android linker still warns
`DT_FLAGS_1=0x8000001`; `NOW|PIE` hardening flags are retained.

Simulated I/O faults cover interrupted commits and checkpoints, but no
physical power-cut test was performed. Cross-process reader coordination is
verified on Windows and the Android shell; remote CI remains unverified because
the configured GitHub Actions API returns 404. FerriteDB has not been
integrated into the WBF backend.

Manifest patch versions are now `ferritedb-core` 0.4.1, `ferritedb` 0.2.1,
`ferrite-pager` 0.5.2, and `ferrite-vfs` 0.2.2. Publication status remains
unconfirmed.
