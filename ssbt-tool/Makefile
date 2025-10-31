SHELL := /bin/bash -e
HASH := $(shell git rev-parse --short HEAD)
RELEASE_DIR := release/$(HASH)
PLATFORMS := linux-x86_64 linux-arm64
BINARY := ssbt-tool

.PHONY: all $(PLATFORMS) clean

all: $(PLATFORMS)

$(PLATFORMS):
	@echo "Building for $@..."
	@mkdir -p $(RELEASE_DIR)/$@
	@case $@ in \
		linux-x86_64)   TARGET=x86_64-unknown-linux-gnu ;; \
		linux-arm64)    TARGET=aarch64-unknown-linux-gnu ;; \
		macos-x86_64)   TARGET=x86_64-apple-darwin ;; \
		macos-arm64)    TARGET=aarch64-apple-darwin ;; \
		windows-x86_64) TARGET=x86_64-pc-windows-gnu ;; \
		windows-arm64)  TARGET=aarch64-pc-windows-gnu ;; \
	esac; \
	cargo build --release --target $$TARGET; \
	cp ../target/$$TARGET/release/$(BINARY)* $(RELEASE_DIR)/$@/; \
	env HASH=$(HASH) envsubst < install_$$(echo $@ | cut -d'-' -f1).sh > $(RELEASE_DIR)/$@/install_$$(echo $@ | cut -d'-' -f1)_$(HASH).sh; \
	chmod +x $(RELEASE_DIR)/$@/install_$$(echo $@ | cut -d'-' -f1)_$(HASH).sh; \
	echo "Built $@ release in $(RELEASE_DIR)/$@"

clean:
	@cargo clean
	@rm -rf release
