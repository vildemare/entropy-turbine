.PHONY: pack-windows

WINDOWS_TARGET := x86_64-pc-windows-gnu
WINDOWS_EXE := target/$(WINDOWS_TARGET)/share/entropy-turbine.exe

# Cross-compile the share build, then zip that exe with assets/.
pack-windows:
	CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc cargo build --profile share --target $(WINDOWS_TARGET)
	cargo run --bin package_windows --features package -- $(WINDOWS_EXE)


sound_gs_shot:
	cargo run -p entropy-synth -- play player_shot

sound_export:
	cargo run -p entropy-synth -- export
