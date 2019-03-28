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

deps: cargo yarn

lint: cargo.lint


fmt: cargo.fmt


# Run all project tests.
#
# Usage:
#	make test

test: test.unit



# Resolve Cargo project dependencies.
#
# Usage:
#	make cargo [cmd=(fetch|<cargo-cmd>)]
#	           [background=(no|yes)]
#	           [dockerized=(no|yes)]

cargo-cmd = $(if $(call eq,$(cmd),),fetch,$(cmd))

cargo:
ifeq ($(dockerized),yes)
ifeq ($(background),yes)
	-@docker stop cargo-cmd
	-@docker rm cargo-cmd
endif
	docker run --rm --user $(shell id -u) --network=host -v "$(PWD)":/app -w /app \
	           --name=cargo-cmd $(if $(call eq,$(background),yes),-d,) \
	           -v "$(abspath $(CARGO_HOME))/registry":/usr/local/cargo/registry\
		rust:$(RUST_VER) \
			make cargo cmd='$(cargo-cmd)' dockerized=no background=no
else
	cargo $(cargo-cmd) $(if $(call eq,$(background),yes),&,)
ifeq ($(cargo-cmd),fetch)
	cargo fetch --manifest-path client/medea-client/Cargo.toml
endif
endif



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
#	          [dockerized=(yes|no)]

yarn-cmd = $(if $(call eq,$(cmd),),fetch,$(cmd))

yarn:
ifneq ($(dockerized),no)
	docker run --rm --user $(shell id -u) --network=host -v "$(PWD)":/app -w /app \
		node:$(NODE_VER) \
			make yarn cmd='$(yarn-cmd)' dockerized=no
else
ifeq ($(yarn-cmd),fetch)
	yarn install --pure-lockfile --cwd client/e2e
else
	yarn $(yarn-cmd)
endif
endif




# Lint Rust sources with clippy.
#
# Usage:
#	make cargo.lint [dockerized=(no|yes)]

cargo.lint:
ifeq ($(dockerized),yes)
	docker run --rm --user $(shell id -u) --network=host -v "$(PWD)":/app -w /app \
	           -v "$(abspath $(CARGO_HOME))/registry":/usr/local/cargo/registry\
		rust:$(RUST_VER) \
			make cargo.lint dockerized=no pre-install=yes
else
ifeq ($(pre-install),yes)
	rustup component add clippy
endif
	cargo clippy -- -D clippy::pedantic -D warnings
endif




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
	wasm-pack test --headless --firefox client/medea-client
endif
endif




# Format Rust sources with rustfmt.
#
# Usage:
#	make cargo.fmt [check=(no|yes)]
#	               [dockerized=(no|yes)]

cargo.fmt:
ifeq ($(dockerized),yes)
	docker pull rustlang/rust:nightly
	docker run --rm --user $(shell id -u) --network=host -v "$(PWD)":/app -w /app \
	           -v "$(abspath $(CARGO_HOME))/registry":/usr/local/cargo/registry\
		rustlang/rust:nightly \
			make cargo.fmt check='$(check)' dockerized=no pre-install=yes
else
ifeq ($(pre-install),yes)
	rustup component add rustfmt
endif
	cargo +nightly fmt --all $(if $(call eq,$(check),yes),-- --check,)
endif





##################
# .PHONY section #
##################

.PHONY: cargo cargo.fmt cargo.lint \
        test test.unit
