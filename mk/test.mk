####################
# Testing commands #
####################

# Run Rust unit tests of project.
#
# Usage:
#	make test.unit [crate=(@all|medea|<crate-name>)]

test-unit-crate = $(if $(call eq,$(crate),),@all,$(crate))

test.unit:
ifeq ($(test-unit-crate),@all)
	@make test.unit crate=medea-macro
	@make test.unit crate=medea-client-api-proto
	@make test.unit crate=medea-jason
	@make test.unit crate=medea
else
ifeq ($(test-unit-crate),medea)
	cargo test --lib --bin medea
else
	cd $(crate-dir)/ && \
	cargo test -p $(test-unit-crate)
endif
endif


# Run medea's signalling tests.
#
# Usage:
#   make test.e2e.signalling [release=(no|yes)] [logs=(no|yes)]

test.e2e.signalling:
ifneq ($(dockerized),no)
	mkdir -p .cache target ~/.cargo/registry
endif
ifneq ($(coturn),no)
	@make up.coturn
endif
ifeq ($(dockerized),no)
	-@make down.medea dockerized=no

	cargo build $(if $(call eq,$(release),yes),--release)
	env $(medea-env) $(if $(call eq,$(logs),yes),,RUST_LOG=warn) cargo run \
		--bin medea $(if $(call eq,$(release),yes),--release) &

	sleep 1
	- cargo test --test e2e

	@make down.medea
else
	-@make down.medea dockerized=yes
	-@make down.medea dockerized=no
	@make docker.build.medea-build

	$(run-medea-container) make test.e2e.signalling dockerized=no coturn=no
endif


# Run e2e tests of medea in chrome.
# If logs set to "yes" then medea print all logs to stdout.
#
# Usage:
# 	make test.e2e.chrome [dockerized=(YES|no)] [logs=(yes|NO)] [coturn=(YES|no)]

test.e2e.chrome:
ifeq ($(dockerized),no)
	@make up.e2e.services
	chromedriver --port=$(chromedriver-port) --log-level=OFF \
		& echo $$! > /tmp/chromedriver.pid

	$(shell cargo run -p e2e-tests-runner -- \
		-w http://localhost:$(chromedriver-port) \
		-f localhost:$(test-runner-port) \
	 	--headless)
	kill $$(cat /tmp/chromedriver.pid)
	rm -f /tmp/chromedriver.pid

	@make down.e2e.services

	@exit $(.SHELLSTATUS)
else
	@make up.e2e.services

	docker run --rm -d --network=host selenoid/chrome:latest > /tmp/chromedriver.docker.uid
	$(run-medea-container) cargo run -p e2e-tests-runner -- \
		-f 127.0.0.1:$(test-runner-port) \
		-w http://127.0.0.1:4444 \
		--headless
	docker container kill $$(cat /tmp/chromedriver.docker.uid)
	rm -f /tmp/chromedriver.docker.uid

	@make down.e2e.services
endif


# Run e2e tests of medea in firefox.
# If logs set to "yes" then medea print all logs to stdout.
#
# Usage:
# 	make test.e2e.firefox [dockerized=(YES|no)] [logs=(yes|NO)] [coturn=(YES|no)]

test.e2e.firefox:
ifeq ($(dockerized),no)
	@make up.e2e.services

	$(shell cargo run -p e2e-tests-runner -- \
		-w http://127.0.0.1:$(geckodriver-port) \
		-f 127.0.0.1:$(test-runner-port) \
		--headless)
	kill $$(cat /tmp/geckodriver.pid)
	rm -f /tmp/geckodriver.pid

	@make down.e2e.services

	@exit $(.SHELLSTATUS)
else
	docker build -t medea-geckodriver -f build/geckodriver/Dockerfile .
	@make up.e2e.services

	docker run --rm -d --network=host medea-geckodriver > /tmp/geckodriver.docker.uid
	$(run-medea-container) cargo run -p e2e-tests-runner -- \
		-f localhost:$(test-runner-port) \
		-w http://localhost:4444 \
		--headless

	docker container kill $$(cat /tmp/geckodriver.docker.uid)
	rm -f /tmp/geckodriver.docker.uid
	@make down.e2e.services
endif
