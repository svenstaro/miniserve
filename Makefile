.PHONY: all
local:
	cargo build --release
	strip target/release/miniserve
	upx target/release/miniserve

run:
ifndef ARGS
	@echo Run "make run" with ARGS set to pass arguments…
endif
	cargo run --release -- $(ARGS)

.PHONY: build-linux
build-linux:
	cargo build --target x86_64-unknown-linux-musl --release
	strip target/x86_64-unknown-linux-musl/release/miniserve
	upx target/x86_64-unknown-linux-musl/release/miniserve

.PHONY: build-win
build-win:
	RUSTFLAGS="-C linker=x86_64-w64-mingw32-gcc" cargo build --target x86_64-pc-windows-gnu --release
	strip target/x86_64-pc-windows-gnu/release/miniserve.exe
	upx target/x86_64-pc-windows-gnu/release/miniserve.exe

.PHONY: build-apple
build-apple:
	cargo build --target x86_64-apple-darwin --release
	strip target/x86_64-apple-darwin/release/miniserve
	upx target/x86_64-apple-darwin/release/miniserve
