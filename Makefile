###############################
# Common defaults/definitions #
###############################

comma := ,

# Checks two given strings for equality.
eq = $(if $(or $(1),$(2)),$(and $(findstring $(1),$(2)),\
                                $(findstring $(2),$(1))),1)




###########
# Aliases #
###########

# Resolve all project dependencies.
#
# Usage:
#	make deps

deps: cargo yarn


docs: docs.rust


lint: cargo.lint


fmt: cargo.fmt


# Run all project application locally in development mode.
#
# Usage:
#	make up

up:
	$(MAKE) -j3 up.coturn up.jason up.medea


test: test.unit




##################
# Cargo commands #
##################

# Resolve Cargo project dependencies.
#
# Usage:
#	make cargo [cmd=(fetch|<cargo-cmd>)]

cargo:
	cargo $(if $(call eq,$(cmd),),fetch,$(cmd))


# Format Rust sources with rustfmt.
#
# Usage:
#	make cargo.fmt [check=(no|yes)]

cargo.fmt:
	cargo +nightly fmt --all $(if $(call eq,$(check),yes),-- --check,)


# Lint Rust sources with clippy.
#
# Usage:
#	make cargo.lint

cargo.lint:
	cargo clippy --all -- -D clippy::pedantic -D warnings




#################
# Yarn commands #
#################

# Resolve NPM project dependencies with Yarn.
#
# Optional 'cmd' parameter may be used for handy usage of docker-wrapped Yarn,
# for example: make yarn cmd='upgrade'
#
# Usage:
#	make yarn [cmd=(install|<yarn-cmd>)]

yarn-cmd =

yarn:
	yarn --cwd=jason/e2e-demo/ $(if $(call eq,$(cmd),),install,$(cmd))




##########################
# Documentation commands #
##########################

# Generate project documentation of Rust sources.
#
# Usage:
#	make docs.rust [open=(yes|no)] [clean=(no|yes)]

docs.rust:
ifeq ($(clean),yes)
	@rm -rf target/doc/
endif
	cargo +nightly doc $(if $(call eq,$(open),no),,--open)




####################
# Testing commands #
####################

# Run Rust unit tests of project.
#
# Usage:
#	make test.unit [crate=(@all|medea|jason|<crate-name>)]

test-unit-crate = $(if $(call eq,$(crate),),@all,$(crate))

test.unit:
ifeq ($(test-unit-crate),@all)
	@make test.unit crate=medea-client-api-proto
	@make test.unit crate=medea-macro
	@make test.unit crate=medea
	@make test.unit crate=jason
else
ifeq ($(test-unit-crate),medea)
	cargo test --lib --bin medea
else
ifeq ($(test-unit-crate),jason)
	wasm-pack test --headless --firefox jason
endif
	cargo test -p $(test-unit-crate)
endif
endif


# Run Rust e2e tests of project.
# If logs set to "yes" then medea print all logs to stdout.
#
# Usage:
# 	make test.e2e [dockerized=(yes|no)] [logs=(yes|no)]

medea-env-dockerized = MEDEA_SERVER_BIND_PORT=8081 \
	MEDEA_SERVER_STATIC_SPECS_PATH=./tests/specs

medea-env-debug = MEDEA_SERVER.BIND_PORT=8081 \
	MEDEA_SERVER.STATIC_SPECS_PATH=./tests/specs

test.e2e: up.coturn
ifeq ($(dockerized),yes)
	make down.medea
	env $(medea-env-dockerized) docker-compose -f docker-compose.medea.yml up -d
	- cargo test --test e2e && make down.medea
else
	- killall medea
	echo $(medea-env)
	env $(medea-env-debug) $(if $(call eq,$(logs),yes),,RUST_LOG=warn) cargo run &
	- sleep 2 && cargo test --test e2e && killall medea
endif




####################
# Running commands #
####################

# Run Coturn STUN/TURN server.
#
# Usage:
#	make up.coturn

up.coturn: down.coturn
	docker-compose -f docker-compose.coturn.yml up -d


# Run Jason E2E demo in development mode.
#
# Usage:
#	make up.jason

up.jason:
	npm run start --prefix=jason/e2e-demo


# Run Medea media server in development mode.
#
# Usage:
#	make up.medea

up.medea: up.coturn
ifeq ($(dockerized),yes)
	@make down.medea
	docker-compose -f docker-compose.medea.yml up
	@make down.coturn
else
	cargo run --bin medea
endif




#####################
# Stopping commands #
#####################

# Stop dockerized medea.
#
# Usage:
# 	make down.medea

down.medea:
	docker-compose -f docker-compose.medea.yml down


# Stop dockerized coturn.
#
# Usage:
# 	make down.coturn

down.coturn:
	docker-compose -f docker-compose.coturn.yml down -t 1




##################
# .PHONY section #
##################

.PHONY: cargo cargo.fmt cargo.lint \
        docs docs.rust \
        test test.unit \
        up up.coturn up.jason up.medea \
        yarn down.coturn down.medea \
        test.e2e

