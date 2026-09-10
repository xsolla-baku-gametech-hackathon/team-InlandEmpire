# make demo            fake taps, mock provider
# make demo PORT=COM3  real pad, provider from .env
# make test            whole workspace
PORT ?=

ifeq ($(OS),Windows_NT)
RUN = powershell -ExecutionPolicy Bypass -File scripts/demo.ps1 $(PORT)
else
RUN = scripts/demo.sh $(PORT)
endif

.PHONY: demo test lint
demo:
	$(RUN)
test:
	cargo test --workspace
lint:
	cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
