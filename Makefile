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


test: test.unit test.e2e




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
# 	make test.e2e [dockerized=(YES|no)] [logs=(yes|NO)] [coturn=(YES|no)]

medea-env = RUST_BACKTRACE=1 \
	MEDEA_SERVER.BIND_PORT=8081 \
	$(if $(call eq,$(logs),yes),,RUST_LOG=warn) \
	MEDEA_SERVER.STATIC_SPECS_PATH=tests/specs

test.e2e:
ifeq ($(coturn),no)
else
	@make up.coturn
endif
ifeq ($(dockerized),no)
	@make down.medea dockerized=no

	cargo build $(if $(call eq,$(release),yes),--release)
	env $(medea-env) $(if $(call eq,$(logs),yes),,RUST_LOG=warn) cargo run --bin medea $(if $(call eq,$(release),yes),--release) &

	sleep 1
	- cargo test --test e2e

	@make down.medea
ifeq ($(coturn),no)
else
	@make down.coturn
endif
else
	@make down.medea dockerized=yes
	@make down.medea dockerized=no
	@make up.coturn

	docker run --rm --network=host -v "$(PWD)":/app -w /app \
			   -v "$(PWD)/.cache/medea/registry":/usr/local/cargo/registry \
			   -v "$(PWD)/.cache/medea/target":/app/target \
		rust:latest \
			make test.e2e dockerized=no coturn=no release=yes

	@make down.coturn
endif




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


# Run Jason E2E demo in development mode.
#
# Usage:
#	make up.jason

up.jason:
	npm run start --prefix=jason/e2e-demo


# Run Medea media server in development mode.
#
# Defaults:
# 	dockerized=no
#
# Usage:
#	make up.medea  [dockerized=(yes|no)]

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

# Stop all related to Medea services.

down:
	@make down.medea dockerized=yes
	@make down.medea dockerized=no
	@make down.coturn


# Stop Medea media server.
#
# Defaults:
# 	dockerized=no
#
# Usage:
# 	make down.medea [dockerized=(yes|no)]

down.medea:
ifeq ($(dockerized),yes)
	docker-compose -f docker-compose.medea.yml down
else
	- killall medea
endif


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
        test.e2e test test.unit \
        up up.coturn up.jason up.medea \
        yarn down down.coturn down.medea

