SHELL := /bin/sh

WAILS_VERSION := $(shell cat .wails-version)
WAILS ?= $(shell go env GOPATH)/bin/wails
GO_PACKAGES := . ./internal/... ./cmd/...

.PHONY: setup setup-wails check-wails fmt fmt-check vet test-go typecheck lint-frontend test-frontend test-frontend-accessibility test-frontend-performance test-frontend-release test-frontend-responsive test-frontend-visual build-frontend check build dmg linux build-current-installer verify-current-installer verify-packaged-acceptance verify-platform-linux smoke-foundation smoke-first-story smoke-release

setup: setup-wails
	go mod download
	npm --prefix frontend ci

setup-wails:
	GOBIN="$(shell go env GOPATH)/bin" go install github.com/wailsapp/wails/v2/cmd/wails@$(WAILS_VERSION)

check-wails:
	@installed="$$($(WAILS) version 2>/dev/null | head -n 1)"; \
	if [ "$$installed" != "$(WAILS_VERSION)" ]; then \
		printf 'Wails CLI at %s must be %s (found: %s). Run make setup-wails.\n' "$(WAILS)" "$(WAILS_VERSION)" "$${installed:-missing}" >&2; \
		exit 1; \
	fi

fmt:
	go fmt $(GO_PACKAGES)

fmt-check:
	go fmt $(GO_PACKAGES)
	git diff --exit-code -- '*.go'

vet:
	go vet $(GO_PACKAGES)

test-go:
	go test $(GO_PACKAGES)

typecheck:
	npm --prefix frontend run typecheck

lint-frontend:
	npm --prefix frontend run lint

test-frontend:
	npm --prefix frontend run test

test-frontend-accessibility:
	npm --prefix frontend run test:accessibility

test-frontend-performance:
	npm --prefix frontend run test:performance

test-frontend-release:
	go test . -run '^TestCanonicalPrototypeContract$$'
	npm --prefix frontend run test:release

test-frontend-responsive:
	npm --prefix frontend run test:responsive

test-frontend-visual:
	npm --prefix frontend run test:visual

build-frontend:
	npm --prefix frontend run build

check: build-frontend fmt-check vet test-go typecheck lint-frontend test-frontend-release test-frontend-performance

build: check-wails build-frontend
	$(WAILS) build -m -nocolour
	find frontend/wailsjs/go -type f -exec chmod 644 {} +

dmg: build-current-installer
	$(MAKE) verify-current-installer

linux:
	scripts/build-linux

build-current-installer: build-frontend
	scripts/build-current-installer

verify-current-installer:
	scripts/verify-current-installer

verify-packaged-acceptance: build-current-installer
	scripts/verify-packaged-acceptance

verify-platform-linux: build-frontend
	scripts/verify-platform-linux

smoke-foundation:
	scripts/smoke-foundation

smoke-first-story:
	go test ./internal/app -run '^TestFirstStoryVerticalSliceThroughPickerAndApplication$$' -v

smoke-release:
	scripts/smoke-release
