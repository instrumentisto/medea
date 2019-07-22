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

MEDEA_IMAGE_NAME := $(strip $(shell grep 'COMPOSE_IMAGE_NAME=' .env | cut -d '=' -f2))
DEMO_IMAGE_NAME := $(strip $(shell grep 'DEMO_IMAGE_NAME=' .env | cut -d '=' -f2))


RUST_VER := 1.36




###########
# Aliases #
###########

build: docker.build.medea


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
	cargo +nightly clippy --all -- -D clippy::pedantic -D warnings




######################
# wasm-pack commands #
######################


# Build and publish Jason application to npm
#
# Usage:
#	make publish.jason

publish.jason:
	rm -rf jason/pkg
	wasm-pack build -t web --no-typescript jason
	wasm-pack publish




#################
# Yarn commands #
#################

# Resolve Yarn project dependencies.
#
# Optional 'cmd' parameter may be used for handy usage of docker-wrapped Yarn,
# for example: make yarn.deps cmd='upgrade'
#
# Usage:
#	make yarn [cmd=('install --pure-lockfile'|<yarn-cmd>)]
#	          [proj=(e2e|demo)]

yarn-cmd = $(if $(call eq,$(cmd),),install --pure-lockfile,$(cmd))
yarn-proj =$(if $(call eq,$(proj),demo),jason/demo,jason/e2e-demo)

yarn:
	docker run --rm --network=host -v "$(PWD)":/app -w /app \
	           -e YARN_CACHE_FOLDER=.cache/yarn/ \
	           -u $(shell id -u):$(shell id -g) \
		node:latest yarn --cwd=$(if $(call eq,$(proj),demo),jason/demo,jason/e2e-demo) \
		$(yarn-cmd) --non-interactive




##########################
# Documentation commands #
##########################

# Generate project documentation of Rust sources.
#
# Usage:
#	make docs.rust [crate=(@all|medea|jason|<crate-name>)]
#	               [open=(yes|no)] [clean=(no|yes)]

docs-rust-crate = $(if $(call eq,$(crate),),@all,$(crate))

docs.rust:
ifeq ($(clean),yes)
	@rm -rf target/doc/
endif
	cargo +nightly doc \
		$(if $(call eq,$(docs-rust-crate),@all),--all,-p $(docs-rust-crate)) \
		--no-deps \
		$(if $(call eq,$(open),no),,--open)




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
	cargo test --bin medea
else
ifeq ($(test-unit-crate),jason)
	wasm-pack test --headless --firefox jason
endif
	cargo test -p $(test-unit-crate)
endif
endif




###################
# Docker commands #
###################

# Build medea project Docker image.
#
# Usage:
#	make docker.build.medea [TAG=(dev|<tag>)]
#	                  [debug=(yes|no)] [no-cache=(no|yes)]

docker-build-medea-image-name = $(MEDEA_IMAGE_NAME)

docker.build.medea:
ifneq ($(no-cache),yes)
	docker run --rm --network=host -v "$(PWD)":/app -w /app \
	           -u $(shell id -u):$(shell id -g) \
	           -e CARGO_HOME=.cache/cargo \
		rust:$(RUST_VER) \
			cargo build --bin=medea \
				$(if $(call eq,$(debug),no),--release,)
endif
	$(call docker.build.clean.ignore)
	@echo "!target/$(if $(call eq,$(debug),no),release,debug)/" >> .dockerignore
	docker build --network=host --force-rm \
		$(if $(call eq,$(no-cache),yes),\
			--no-cache --pull,) \
		$(if $(call eq,$(IMAGE),),\
			--build-arg rust_ver=$(RUST_VER) \
			--build-arg rustc_mode=$(if \
				$(call eq,$(debug),no),release,debug) \
			--build-arg rustc_opts=$(if \
				$(call eq,$(debug),no),--release,) \
			--build-arg cargo_home=.cache/cargo,) \
		-t $(docker-build-medea-image-name):$(if $(call eq,$(TAG),),dev,$(TAG)) .
	$(call docker.build.clean.ignore)
define docker.build.clean.ignore
	@sed -i $(if $(call eq,$(shell uname -s),Darwin),'',) \
		/^!target\/d .dockerignore
endef




# Build demo project Docker image.
#
# Usage:
#	make docker.build.demo [TAG=(dev|<tag>)]

docker-build-demo-image-name = $(DEMO_IMAGE_NAME)

docker.build.demo:
	@make yarn proj=demo
	docker build -t $(docker-build-demo-image-name):$(if $(call eq,$(TAG),),dev,$(TAG)) jason/demo



####################
# Running commands #
####################

# Run Coturn STUN/TURN server.
#
# Usage:
#	make up.coturn

up.coturn:
	docker-compose up


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

.PHONY: build cargo cargo.fmt cargo.lint \
        docker.build.medea \
        docker.build.demo \
        docs docs.rust \
        test test.unit \
        up up.coturn up.jason up.medea \
        yarn

