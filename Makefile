DESTDIR ?= $(HOME)/go
PREFIX ?=
BINDIR = $(PREFIX)/bin

build:
	@goreleaser build --snapshot --single-target --clean

test:
	@go test -race -covermode=atomic -timeout 5s ./...

install:
	@install -v -D -m 755 dist/extension-downloader_linux_amd64_v1/extension-downloader $(DESTDIR)$(BINDIR)/extension-downloader

clean:
	@$(RM) -r dist

.PHONY: build test install clean
