# Include all Makefile modules.
include mk/*.mk

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

MEDEA_IMAGE_NAME := $(strip \
	$(shell grep 'COMPOSE_IMAGE_NAME=' .env | cut -d '=' -f2))
DEMO_IMAGE_NAME := instrumentisto/medea-demo

RUST_VER := 1.36

CURRENT_BRANCH := $(strip $(shell git branch | grep \* | cut -d ' ' -f2))

crate-dir = .
ifeq ($(crate),medea-jason)
crate-dir = jason
endif
ifeq ($(crate),medea-client-api-proto)
crate-dir = proto/client-api
endif
ifeq ($(crate),medea-macro)
crate-dir = crates/medea-macro
endif




###########
# Aliases #
###########

build: docker.build.medea build.medea build.jason


# Resolve all project dependencies.
#
# Usage:
#	make deps

deps: cargo yarn


docs: docs.rust


fmt: cargo.fmt


lint: cargo.lint


# Build and publish project crate everywhere.
#
# Usage:
#	make release crate=(medea|medea-jason|<crate-name>)
#	             [publish=(no|yes)]

release: release.crates release.npm


test: test.unit test.e2e


# Run all E2E tests of medea.
#
# Usage:
# 	make test.e2e [dockerized=(YES|no)] [logs=(yes|NO)] [coturn=(YES|no)]
test.e2e: test.e2e.signalling test.e2e.chrome test.e2e.firefox




#################
# Yarn commands #
#################

# Resolve NPM project dependencies with Yarn.
#
# Optional 'cmd' parameter may be used for handy usage of docker-wrapped Yarn,
# for example: make yarn cmd='upgrade'
#
# Usage:
#	make yarn [cmd=('install --pure-lockfile'|<yarn-cmd>)]
#	          [proj=(e2e|demo)]
#	          [dockerized=(yes|no)]

yarn-cmd = $(if $(call eq,$(cmd),),install --pure-lockfile,$(cmd))
yarn-proj-dir = $(if $(call eq,$(proj),demo),jason/demo,jason/e2e-demo)

yarn:
ifneq ($(dockerized),no)
	docker run --rm --network=host -v "$(PWD)":/app -w /app \
	           -u $(shell id -u):$(shell id -g) \
		node:latest \
			make yarn cmd='$(yarn-cmd)' proj=$(proj) dockerized=no
else
	yarn --cwd=$(yarn-proj-dir) $(yarn-cmd)
endif




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




##################
# .PHONY section #
##################

.PHONY: build \
        cargo cargo.fmt cargo.lint \
        docker.build.demo docker.build.medea docker.down.demo docker.up.demo \
        docker.build.medea-build \
        docs docs.rust \
        down.demo \
        helm helm.down helm.init helm.lint helm.list \
        	helm.package helm.package.release helm.up \
        minikube.boot \
        test test.unit \
        test.e2e test.e2e.chrome test.e2e.firefox test.e2e.signalling \
        down down.medea down.coturn down.e2e.services \
        release release.crates release.helm release.npm \
        up up.coturn up.demo up.dev up.jason up.medea up.e2e.services \
        build.medea build.jason \
        yarn
