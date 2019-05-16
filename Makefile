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

build: build.jason

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
	$(MAKE) -j2 up.jason up.medea


test: test.unit




##################
# Build commands #
##################

# Build Jason E2E demo in production mode.
#
# Usage:
#	make build.jason

build.jason:
	npm run build --prefix=jason/e2e-demo
	@make opt.jason


# Optimize wasm binary.
#
# Usage:
#	make opt.wasm [filename=(|<wasm-file>)]

wasm-file = $(if $(call eq,$(filename),),$(shell find jason/e2e-demo/dist -name '*.module.wasm'),$(filename))

opt.jason:
	wasm-opt $(wasm-file) -o $(wasm-file)




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
#	make test.unit [app=(all|medea|jason)]

test-unit-app = $(if $(call eq,$(app),),all,$(app))

test.unit:
ifeq ($(test-unit-app),all)
	@make test.unit app=medea
	@make test.unit app=jason
endif
ifeq ($(test-unit-app),medea)
	cargo test --bin medea
endif
ifeq ($(test-unit-app),jason)
	wasm-pack test --headless --firefox jason
	cargo test -p jason
endif




####################
# Running commands #
####################

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

up.medea:
	cargo run --bin medea




##################
# .PHONY section #
##################

.PHONY: build build.jason \
        cargo cargo.fmt cargo.lint \
        docs docs.rust \
        opt.jason \
        test test.unit \
        up up.jason up.medea \
        yarn

