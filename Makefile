default: build

all: test

test: build
	$(MAKE) -C core test
	$(MAKE) -C orchestrator test

# Refresh Soroban contract test snapshots after an SDK upgrade.
# Usage: make update-snapshots
update-snapshots:
	UPDATE_SNAPSHOTS=1 $(MAKE) -C core test

build:
	$(MAKE) -C core build
	$(MAKE) -C orchestrator build

check-size:
	$(MAKE) -C core check-size

fmt:
	$(MAKE) -C core fmt
	$(MAKE) -C orchestrator fmt

clean:
	$(MAKE) -C core clean
	$(MAKE) -C orchestrator clean
