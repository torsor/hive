# hive — local dev: build release artifacts into bin/, test, optional PATH symlinks.
REPO_ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
BIN_DIR := $(REPO_ROOT)/bin
CARGO_TARGET := $(REPO_ROOT)/target/release
PANEL_DIR := $(REPO_ROOT)/apps/hive-panel
PANEL_BIN := $(PANEL_DIR)/src-tauri/target/release/hive-panel
INSTALL_BIN ?= $(HOME)/.local/bin

.DEFAULT_GOAL := help

WEB_DIR := $(REPO_ROOT)/apps/hive-web

.PHONY: help check test test-integration stack-smoke panel-check web-check deploy-web \
	build build-cli build-host build-hub build-web build-panel \
	install install-binaries install-cli install-host install-hub install-web install-panel \
	install-ops

help:
	@echo "Hive — local dev targets"
	@echo ""
	@echo "Build (release artifacts; not copied to bin/ until install):"
	@echo "  make build            hive, hive-host, hive-hub"
	@echo "  make build-cli        hive CLI only"
	@echo "  make build-host       hive-host only"
	@echo "  make build-hub        hive-hub only"
	@echo "  make build-web        hive-web static server + phone SPA dist"
	@echo "  make build-panel      hive-panel (Tauri release; needs node)"
	@echo ""
	@echo "Install (copy into $(BIN_DIR)/ — no PATH symlinks):"
	@echo "  make install          install-binaries (hive, hive-host, hive-hub)"
	@echo "  make install-binaries copy all workspace Rust binaries to bin/"
	@echo "  make install-cli      hive -> bin/hive"
	@echo "  make install-host     hive-host -> bin/hive-host"
	@echo "  make install-hub      hive-hub -> bin/hive-hub"
	@echo "  make install-web      hive-web -> bin/hive-web"
	@echo "  make install-panel    build-panel, then hive-panel -> bin/hive-panel"
	@echo ""
	@echo "PATH symlinks (optional; separate from repo bin/ install):"
	@echo "  make install-ops      hive-deploy -> $(INSTALL_BIN)/hive-deploy"
	@echo ""
	@echo "Check / test:"
	@echo "  make check            cargo check --workspace"
	@echo "  make test             cargo test --workspace"
	@echo "  make test-integration HTTP/tmux/transcript/install integration tests"
	@echo "  make stack-smoke       local host+hub smoke (scripts/dev-stack.sh)"
	@echo "  make panel-check       hive-panel tsc, vitest, tauri unit tests"
	@echo "  make web-check         hive-web vitest + cargo test -p hive-web"
	@echo "  make deploy-web        ansible --tags web --limit hive_hub (phone SPA)"

build: build-cli build-host build-hub build-web

build-web: build-web-bin build-web-spa

build-web-bin:
	cargo build --release -p hive-web

build-web-spa:
	@cd "$(WEB_DIR)" && { [ -d node_modules ] || npm install; }
	@cd "$(WEB_DIR)" && npm run build

build-cli:
	cargo build --release -p hive-cli

build-host:
	cargo build --release -p hive-host

build-hub:
	cargo build --release -p hive-hub

build-panel:
	@cd "$(PANEL_DIR)" && { [ -d node_modules ] || npm install; }
	@cd "$(PANEL_DIR)" && npm run tauri build

install: install-binaries

install-binaries: build
	@mkdir -p "$(BIN_DIR)"
	@cp "$(CARGO_TARGET)/hive" "$(CARGO_TARGET)/hive-host" "$(CARGO_TARGET)/hive-hub" "$(CARGO_TARGET)/hive-web" "$(BIN_DIR)/"
	@chmod +x "$(BIN_DIR)/hive" "$(BIN_DIR)/hive-host" "$(BIN_DIR)/hive-hub" "$(BIN_DIR)/hive-web" "$(BIN_DIR)/hive-deploy"
	@echo "ok: bin/hive bin/hive-host bin/hive-hub bin/hive-web (bin/hive-deploy unchanged)"

install-cli: build-cli
	@mkdir -p "$(BIN_DIR)"
	@cp "$(CARGO_TARGET)/hive" "$(BIN_DIR)/hive"
	@chmod +x "$(BIN_DIR)/hive"
	@echo "ok: bin/hive"

install-host: build-host
	@mkdir -p "$(BIN_DIR)"
	@cp "$(CARGO_TARGET)/hive-host" "$(BIN_DIR)/hive-host"
	@chmod +x "$(BIN_DIR)/hive-host"
	@echo "ok: bin/hive-host"

install-hub: build-hub
	@mkdir -p "$(BIN_DIR)"
	@cp "$(CARGO_TARGET)/hive-hub" "$(BIN_DIR)/hive-hub"
	@chmod +x "$(BIN_DIR)/hive-hub"
	@echo "ok: bin/hive-hub"

install-web: build-web-bin
	@mkdir -p "$(BIN_DIR)"
	@cp "$(CARGO_TARGET)/hive-web" "$(BIN_DIR)/hive-web"
	@chmod +x "$(BIN_DIR)/hive-web"
	@echo "ok: bin/hive-web"

install-panel: build-panel
	@mkdir -p "$(BIN_DIR)"
	@cp "$(PANEL_BIN)" "$(BIN_DIR)/hive-panel"
	@chmod +x "$(BIN_DIR)/hive-panel"
	@echo "ok: bin/hive-panel"

install-ops:
	@mkdir -p "$(INSTALL_BIN)"
	@chmod +x "$(BIN_DIR)/hive-deploy"
	@ln -sf "$(BIN_DIR)/hive-deploy" "$(INSTALL_BIN)/hive-deploy"
	@echo "ok: hive-deploy -> $(INSTALL_BIN)/hive-deploy"

check:
	cargo check --workspace

test:
	cargo test --workspace

test-integration:
	cargo test --workspace --test http --test tmux --test transcript --test install -p hive-host -p hive-client -p hive-cli

stack-smoke:
	@./scripts/dev-stack.sh smoke

panel-check:
	cd apps/hive-panel && npm run check && npm test
	cd apps/hive-panel/src-tauri && cargo test

web-check:
	cd apps/hive-web && npm run test
	cargo test -p hive-web
	cargo test -p hive-common

deploy-web:
	cd ansible && ansible-playbook site.yml -K --tags web --limit hive_hub
