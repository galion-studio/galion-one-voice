# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Release build for the Samantha user package.
# The crate is a member of voice-stack/Cargo.toml, not of the repository
# workspace. Artifacts stay in this directory so the install scripts find them.

SECTOR := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
CARGO ?= cargo
export CARGO_TARGET_DIR := $(SECTOR)/target
VERSION := 0.1.0
HOST := $(shell rustc -vV | awk '/^host:/{print $$2}')
NAME := galion-samantha-$(VERSION)-$(HOST)
STAGE := $(SECTOR)/dist/$(NAME)

.PHONY: release debug test check shellcheck verify-unit install dry-run dist

release: dist

dist:
	$(CARGO) build --release --locked --manifest-path $(SECTOR)/Cargo.toml
	rm -rf $(STAGE)
	mkdir -p $(STAGE)/bin $(STAGE)/lib $(STAGE)/share $(STAGE)/ci $(STAGE)/tests $(STAGE)/src
	cp $(SECTOR)/target/release/galion-samantha $(STAGE)/bin/galion-samantha
	cp $(SECTOR)/install.sh $(SECTOR)/uninstall.sh $(SECTOR)/Makefile $(SECTOR)/LICENSE $(SECTOR)/README.md $(SECTOR)/Cargo.toml $(SECTOR)/Cargo.lock $(STAGE)/
	cp -a $(SECTOR)/lib/. $(STAGE)/lib/
	cp -a $(SECTOR)/share/. $(STAGE)/share/
	cp -a $(SECTOR)/ci/. $(STAGE)/ci/
	cp -a $(SECTOR)/tests/. $(STAGE)/tests/
	cp -a $(SECTOR)/src/. $(STAGE)/src/
	tar -C $(SECTOR)/dist -cJf $(SECTOR)/dist/$(NAME).tar.xz $(NAME)
	cd $(SECTOR)/dist && sha256sum $(NAME).tar.xz > SHA256SUMS
	@echo "release $(SECTOR)/dist/$(NAME).tar.xz"

debug:
	$(CARGO) build --manifest-path $(SECTOR)/Cargo.toml

test: debug
	$(CARGO) test --manifest-path $(SECTOR)/Cargo.toml
	$(SECTOR)/tests/install_test.sh

shellcheck:
	$(SECTOR)/ci/shellcheck.sh

verify-unit:
	$(SECTOR)/ci/verify-unit.sh

check: shellcheck verify-unit test

dry-run:
	$(SECTOR)/install.sh --dry-run

install: release
	$(SECTOR)/install.sh
