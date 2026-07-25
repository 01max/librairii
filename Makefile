SHELL := /bin/sh

WAILS_VERSION := $(shell cat .wails-version)
WAILS ?= $(shell go env GOPATH)/bin/wails
GO_PACKAGES := . ./internal/... ./cmd/...

.PHONY: setup fmt fmt-check vet test-go typecheck lint-frontend test-frontend test-frontend-performance build-frontend check build smoke-foundation smoke-first-story

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

test-frontend-performance:
	npm --prefix frontend run test:performance

build-frontend:
	npm --prefix frontend run build

check: fmt-check vet test-go typecheck lint-frontend test-frontend test-frontend-performance build-frontend

build:
	test "$$($(WAILS) version | head -n 1)" = "$(WAILS_VERSION)"
	$(WAILS) build -m -nocolour
	find frontend/wailsjs/go -type f -exec chmod 644 {} +

smoke-foundation:
	scripts/smoke-foundation

smoke-first-story:
	go test ./internal/app -run '^TestFirstStoryVerticalSliceThroughPickerAndApplication$$' -v
