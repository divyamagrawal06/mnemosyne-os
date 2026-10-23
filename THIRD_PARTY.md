# Third-party code and dependencies

MnemosyneOS reuses upstream implementations through Cargo where possible. `Cargo.lock` records exact resolved packages and registry checksums. Original project code is MIT licensed; that does not replace upstream licenses.

## Copied and adapted files

| Origin | Pinned revision | Local use | License and changes |
| --- | --- | --- | --- |
| [Limine Rust template](https://github.com/jasondyoungberg/limine-rust-template/tree/ce82375f39a8e8ae85fbee1d1b147290cd6670c8) | `ce82375f39a8e8ae85fbee1d1b147290cd6670c8` | `kernel/linker-x86_64.ld`, boot request layout in `kernel/src/main.rs`, `kernel/build.rs` | 0BSD. Linker script copied; boot example adapted to serial/memory diagnostics; build script uses an absolute path and checks the target. Notice retained in `third-party/licenses/limine-rust-template-0BSD.txt`. |
| [Limine binary distribution](https://github.com/limine-bootloader/limine/tree/7a9013f305de0bea1f6310e76e7baba30499fef0) | `v10.8.5-binary`, commit `7a9013f305de0bea1f6310e76e7baba30499fef0` | Downloaded to `.tools/limine`; BIOS/UEFI boot files included in the generated ISO | BSD-2-Clause. Unmodified boot files; installer compiled from upstream `limine.c`. License retained locally and included in the ISO. |

The ignored `.tools` directory is a build cache. The runner verifies the bootloader commit before image construction. No downloaded template build script is executed; the project runner builds the selected Limine installer target.

## Direct Cargo dependencies

| Package | Version | Upstream | Declared license | Purpose |
| --- | --- | --- | --- | --- |
| limine | 0.5.0 | [limine-rs](https://github.com/limine-bootloader/limine-rs) | MIT OR Apache-2.0 | Boot protocol structures matching the reused template |
| x86_64 | 0.15.4 | [rust-osdev/x86_64](https://github.com/rust-osdev/x86_64) | MIT / Apache-2.0 | Descriptors, interrupt tables, CPU instructions, port I/O |
| uart_16550 | 0.8.1 | [rust-osdev/uart_16550](https://github.com/rust-osdev/uart_16550) | MIT OR Apache-2.0 | Serial driver |
| pic8259 | 0.11.0 | [rust-osdev/pic8259](https://github.com/rust-osdev/pic8259) | Apache-2.0 / MIT | Legacy PIC initialization and interrupt acknowledgment |
| spin | 0.12.3 | [zesterer/spin](https://codeberg.org/zesterer/spin) | MIT | Allocation-free synchronization and lazy static initialization |
| bitflags | 2.13.2 | [bitflags/bitflags](https://github.com/bitflags/bitflags) | MIT OR Apache-2.0 | Typed capability rights |
| wasmi | 2.0.0 | [wasmi-labs/wasmi](https://github.com/wasmi-labs/wasmi) | MIT / Apache-2.0 | Wasm validation, interpretation, fuel metering, and resource limits in the host replay experiment |
| wat | 1.228.0 | [bytecodealliance/wasm-tools](https://github.com/bytecodealliance/wasm-tools) | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Compiling the original WAT test workload; matches Wasmi's parser generation |
| sha2 | 0.10.9 | [RustCrypto/hashes](https://github.com/RustCrypto/hashes) | MIT OR Apache-2.0 | Module and checkpoint integrity digests |
| serde | 1.0.228 | [serde-rs/serde](https://github.com/serde-rs/serde) | MIT OR Apache-2.0 | Structured experiment output |
| serde_json | 1.0.151 | [serde-rs/json](https://github.com/serde-rs/json) | MIT OR Apache-2.0 | JSON report encoding |
| font8x8 | 0.3.1 | [font8x8-rs](https://gitlab.com/saibatizoku/font8x8-rs) | MIT; underlying VGA bitmap glyphs public domain | Unmodified bitmap font data and lookup API used by the native guest desktop, with `std` disabled |

The source research cache is not incorporated into the implementation. Research references such as seL4, Theseus, RedLeaf, Golem, and AgentSpec are not vendored dependencies. Reuse decisions are made per component; a public repository is not automatically permission to redistribute its code under MIT.

The Wasmi embedding uses upstream APIs without a fork or copied interpreter. Validation, portable dispatch, deterministic execution, fuel, and extra checks are enabled explicitly. The test workload and replay protocol are original code. Cargo dependencies include their upstream license files in the registry packages; retain those notices if distributing the packages or binaries.

QEMU, xorriso, Git, the Rust toolchain, and the C compiler are external development tools. Their code is not compiled into the kernel. The first tested boot path uses QEMU's BIOS/SeaBIOS; the ISO also contains Limine's UEFI files, but that alone does not establish a tested UEFI boot.

## Guest integration dependencies

The pinned `linked_list_allocator` 0.10.6 from
[rust-osdev/linked-list-allocator](https://github.com/rust-osdev/linked-list-allocator)
is reused unchanged for kernel and private userspace heaps (MIT OR Apache-2.0).
Wasmi 2.0.0 now runs in both the host lab and guest init through the shared
`crates/service-runtime` crate, using `no_std`, software math and B-tree collections.
SHA-256 uses `force-soft` because the freestanding CPU ABI excludes SIMD context.
The WAT fixture is compiled at build time with the same pinned `wat` version.

The kernel scheduler/trap ABI, ATA integration, ChronicleFS protocol, COW integration,
policy verifier and guest shell are original project code. ELF page mapping reuses
the upstream `x86_64` crate; the allocator and Wasmi engine are not forked.
`tools/package-demo.py` includes registry dependency license/notice files and
records declared license expressions in the bundle manifest.

Some published Wasmi/wasm-tools crates omit repository-root license files. Their
notices are retained in `third-party/licenses` from the exact revisions recorded
in the crates' `.cargo_vcs_info.json`; `upstream-notices.json` records source URLs,
blob identities and digests. pic8259's pinned README retains its licensing and
authorship statement; its Apache-2.0 option is distributed with the standard
Apache text. The packager fails if a dependency has no retained notice.
