default: build

all: test

test: build
	$(MAKE) -C core test
	$(MAKE) -C orchestrator test

build:
	$(MAKE) -C core build
	$(MAKE) -C orchestrator build

check-size:
	$(MAKE) -C core check-size

fmt:
	$(MAKE) -C core fmt
	$(MAKE) -C orchestrator fmt

clippy:
	$(MAKE) -C core clippy
	$(MAKE) -C orchestrator clippy

check:
	$(MAKE) -C core check
	$(MAKE) -C orchestrator check

doc:
	$(MAKE) -C core doc
	$(MAKE) -C orchestrator doc

clean:
	$(MAKE) -C core clean
	$(MAKE) -C orchestrator clean

.PHONY: default all test build check-size fmt clippy check doc clean
