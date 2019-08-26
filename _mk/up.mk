####################
# Running commands #
####################

# Run Coturn STUN/TURN server.
#
# Defaults:
# 	logs=no
#
# Usage:
#	make up.coturn [logs=(yes|no)]

up.coturn:
	docker-compose -f docker-compose.coturn.yml up -d
ifeq ($(logs),yes)
	docker-compose -f docker-compose.coturn.yml logs &
endif


up.demo: docker.up.demo


# Run Medea and Jason development environment.
#
# Usage:
#	make up.dev

up.dev:
	$(MAKE) -j4 up.coturn up.jason up.medea up.control-api-mock


# Run Jason E2E demo in development mode.
#
# Usage:
#	make up.jason

up.jason:
	npm run start --prefix=jason/e2e-demo



# Run Medea media server in development mode.
# jq flag will redirect medea logs to jq (command-line JSON processor) tool.
#
# Defaults:
# 	dockerized=no
#
# Usage:
#	make up.medea  [dockerized=(yes|no)]
# 				   [jq=(no|yes)]
#				   [jq-args=""]
#				   [watch=(no|yes)]

jq-start = $(if $(call eq,$(jq),yes),| jq -R 'fromjson?' $(jq-args))

up.medea: up.coturn
ifeq ($(dockerized),yes)
	@make down.medea
	docker-compose -f docker-compose.medea.yml up
	@make down.coturn
else
ifeq ($(watch),yes)
	cargo watch -x "run --bin medea" $(jq-start)
else
	cargo run --bin medea $(jq-start)
endif
endif


# Start services needed for e2e tests of medea in browsers.
# If logs set to "yes" then medea print all logs to stdout.
#
# Usage:
# 	make test.e2e [dockerized=(YES|no)] [logs=(yes|NO)] [coturn=(YES|no)]

medea-env = RUST_BACKTRACE=1 \
	MEDEA_SERVER.HTTP.BIND_PORT=8081 \
	$(if $(call eq,$(logs),yes),,RUST_LOG=warn) \
	MEDEA_SERVER.HTTP.STATIC_SPECS_PATH=tests/specs

chromedriver-port = 50000
geckodriver-port = 50001
test-runner-port = 51000

run-medea-command = docker run --rm --network=host -v "$(PWD)":/app -w /app \
                    	--env XDG_CACHE_HOME=$(HOME) \
                    	--env RUST_BACKTRACE=1 \
						-u $(shell id -u):$(shell id -g) \
                    	-v "$(HOME)/.cargo/registry":/usr/local/cargo/registry \
                    	-v "$(HOME):$(HOME)" \
                    	-v "$(PWD)/target":/app/target
run-medea-container-d =  $(run-medea-command) -d medea-build:latest
run-medea-container = $(run-medea-command) medea-build:latest

up.e2e.services:
ifneq ($(dockerized),no)
	mkdir -p .cache target ~/.cargo/registry
endif
ifneq ($(coturn),no)
	@make up.coturn
endif
ifeq ($(dockerized),no)
	cargo build $(if $(call eq,$(release),yes),--release)
	cargo build -p control-api-mock
	$(run-medea-container) sh -c "cd jason && wasm-pack build --target web --out-dir ../.cache/jason-pkg"

	env $(if $(call eq,$(logs),yes),,RUST_LOG=warn) cargo run --bin medea \
		$(if $(call eq,$(release),yes),--release) & \
		echo $$! > /tmp/e2e_medea.pid
	env RUST_LOG=warn cargo run -p control-api-mock & \
		echo $$! > /tmp/e2e_control_api_mock.pid
	sleep 2
else
	@make down.medea dockerized=yes
	@make down.medea dockerized=no
	@make up.coturn

	# TODO: publish it to docker hub
	@make docker.build.medea-build

	$(run-medea-container) sh -c "cd jason && RUST_LOG=info wasm-pack build --target web --out-dir ../.cache/jason-pkg"

	$(run-medea-container) make build.medea optimized=yes
	$(run-medea-container-d) cargo run --release > /tmp/medea.docker.uid

	$(run-medea-container) cargo build -p control-api-mock
	$(run-medea-container-d) cargo run -p control-api-mock > /tmp/control-api-mock.docker.uid

	$(run-medea-container) cargo build -p e2e-tests-runner
endif

up.control-api-mock:
	cargo run -p control-api-mock