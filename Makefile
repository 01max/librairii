SHELL := /bin/sh

WAILS_VERSION := $(shell cat .wails-version)
WAILS ?= $(shell go env GOPATH)/bin/wails
GO_PACKAGES := . ./internal/... ./cmd/...

.PHONY: setup fmt fmt-check vet test-go typecheck lint-frontend test-frontend test-frontend-accessibility test-frontend-performance test-frontend-release test-frontend-responsive test-frontend-visual build-frontend check build build-current-installer verify-current-installer verify-packaged-acceptance verify-platform-linux verify-platform-windows smoke-foundation smoke-first-story smoke-release

setup:
	go mod download
	npm --prefix frontend ci

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

build: build-frontend
	test "$$($(WAILS) version | head -n 1)" = "$(WAILS_VERSION)"
	$(WAILS) build -m -nocolour
	find frontend/wailsjs/go -type f -exec chmod 644 {} +

build-current-installer: build-frontend
	scripts/build-current-installer

verify-current-installer:
	scripts/verify-current-installer

verify-packaged-acceptance: build-current-installer
	scripts/verify-packaged-acceptance

verify-platform-linux: build-frontend
	scripts/verify-platform-linux

verify-platform-windows: build-frontend
	pwsh -NoLogo -NoProfile -File scripts/verify-platform-windows.ps1

smoke-foundation:
	scripts/smoke-foundation

smoke-first-story:
	go test ./internal/app -run '^TestFirstStoryVerticalSliceThroughPickerAndApplication$$' -v

smoke-release:
	scripts/smoke-release
