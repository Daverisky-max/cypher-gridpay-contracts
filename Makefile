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

clean:
	$(MAKE) -C core clean
	$(MAKE) -C orchestrator clean
