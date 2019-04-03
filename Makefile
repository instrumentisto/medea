###############################
# Common defaults/definitions #
###############################

comma := ,

# Checks two given strings for equality.
eq = $(if $(or $(1),$(2)),$(and $(findstring $(1),$(2)),\
								$(findstring $(2),$(1))),1)




######################
# Project parameters #
######################

CARGO_HOME ?= $(strip $(shell dirname $$(dirname $$(which cargo))))
RUST_VER ?= "1.33"
NODE_VER ?= "11.10"




###########
# Aliases #
###########

# Resolve all project dependencies.
#
# Usage:
#	make deps

deps: cargo.deps yarn


fmt: cargo.fmt


lint: cargo.lint


up: up.dev


# Run all project tests.
#
# Usage:
#	make test

test: test.unit



# Resolve Cargo project dependencies.
#
# Usage:
#	make cargo [cmd=(fetch|<cargo-cmd>)]

cargo-cmd = $(if $(call eq,$(cmd),),fetch,$(cmd))

cargo.deps:
	cargo fetch



#################
# Yarn commands #
#################

# Resolve NPM project dependencies with Yarn.
#
# Optional 'cmd' parameter may be used for handy usage of docker-wrapped Yarn,
# for example: make yarn cmd='upgrade'
#
# Usage:
#	make yarn [cmd=('fetch'|<yarn-cmd>)]
#			  [dockerized=(yes|no)]

yarn-cmd = $(if $(call eq,$(cmd),),fetch,$(cmd))

yarn:
ifneq ($(dockerized),no)
	docker run --rm --user $(shell id -u) --network=host -v "$(PWD)":/app -w /app \
		node:$(NODE_VER) \
			make yarn cmd='$(yarn-cmd)' dockerized=no
else
ifeq ($(yarn-cmd),fetch)
	yarn install --pure-lockfile --cwd jason/e2e
else
	yarn $(yarn-cmd)
endif
endif




# Lint Rust sources with clippy.
#
# Usage:
#	make cargo.lint

cargo.lint:
ifeq ($(pre-install),yes)
	rustup component add clippy
endif
	cargo clippy -- -D clippy::pedantic -D warnings




# Run Rust unit tests of project.
#
# Usage:
#	make test.unit [dockerized=(no|yes)] [app=(server|client)]

test.unit:
ifeq ($(app),)
	make test.unit dockerized=$(dockerized) app=server
	make test.unit dockerized=$(dockerized) app=client
endif
ifeq ($(dockerized),yes)
ifeq ($(app),server)
		docker run --rm --user $(shell id -u) --network=host -v "$(PWD)":/app -w /app \
					-v "$(abspath $(CARGO_HOME))/registry":/usr/local/cargo/registry\
			rust:$(RUST_VER) \
				make test.unit dockerized=no app=server
endif
ifeq ($(app),client)
		docker run --rm --user $(shell id -u) --network=host -v "$(PWD)":/app -w /app \
					-v "$(abspath $(CARGO_HOME))/registry":/usr/local/cargo/registry\
			alexlapa/wasm-pack:stable-$(RUST_VER)-ff-66.0 \
				make test.unit dockerized=no app=client
endif
else
ifeq ($(app),server)
	cargo test --all
endif
ifeq ($(app),client)
	wasm-pack test --headless --firefox jason
endif
endif




# Format Rust sources with rustfmt.
#
# Usage:
#	make cargo.fmt [check=(no|yes)]

cargo.fmt:
ifeq ($(pre-install),yes)
	rustup component add rustfmt
endif
	cargo +nightly fmt --all $(if $(call eq,$(check),yes),-- --check,)




# Run projects Medea and e2e app locally with dev settings.
#
# Usage:
#	make up.dev

up.dev:
	$(MAKE) -j2 up.dev.server up.dev.e2e

up.dev.server:
	cargo run

up.dev.e2e:
	npm run start --prefix jason/e2e




##################
# .PHONY section #
##################

.PHONY: cargo.deps cargo.fmt cargo.lint \
		fmt lint test test.unit \
		up up.dev up.dev.server up.dev.e2e yarn
