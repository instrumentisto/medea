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


test: test.unit test.e2e




####################
# Running commands #
####################

down.demo: docker.down.demo


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
	$(MAKE) -j3 up.coturn up.jason up.medea


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
	docker-compose -f docker-compose.coturn.yml down




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
else
	cargo test -p $(test-unit-crate)
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
ifneq ($(coturn),no)
	@make up.coturn
endif
ifeq ($(dockerized),no)
	@make down.medea dockerized=no

	cargo build $(if $(call eq,$(release),yes),--release)
	env $(medea-env) $(if $(call eq,$(logs),yes),,RUST_LOG=warn) cargo run $(if $(call eq,$(release),yes),--release) &

	sleep 1
	- cargo test --test e2e

	@make down.medea
ifneq ($(coturn),no)
	@make down.coturn
endif
else
	@make down.medea dockerized=yes
	@make down.medea dockerized=no
	@make up.coturn

	docker run --rm --network=host -v "$(PWD)":/app -w /app \
				-u $(shell id -u):$(shell id -g) \
				-v "$(HOME)/.cargo/registry":/usr/local/cargo/registry \
			   	-v "$(PWD)/target":/app/target \
		rust:latest \
			make test.e2e dockerized=no coturn=no release=yes

	@make down.coturn
endif




######################
# Releasing commands #
######################

# Build and publish Jason to NPM and crates.io.
#
# Note that this command will use CARGO_TOKEN and NPM_TOKEN enviroment
# variables for publishing.
#
# Usage:
#   make release.jason

release.jason: release.npm.jason release.crates.jason


# Build and publish Jason application to npm
#
# Note that this command will use NPM_TOKEN enviroment
# variable for publishing.
#
# Usage:
#	make release.jason

release.npm.jason:
	@rm -rf jason/pkg/
	wasm-pack build -t web jason
	wasm-pack publish


# Build and publish Jason to crates.io
#
# Note that this command will use CARGO_TOKEN enviroment
# variable for publishing.
#
# Usage:
#   make release.crates.jason

release.crates.jason:
	cd jason && cargo publish --token ${CARGO_TOKEN}


# Build and publish Medea to crates.io
#
# Note that this command will use CARGO_TOKEN enviroment
# variable for publishing.
#
# Usage:
#   make release.crates.medea

release.crates.medea:
	cargo publish --token ${CARGO_TOKEN}


# Build and publish Medea client API proto to crates.io
#
# Note that this command will use CARGO_TOKEN enviroment
# variable for publishing.
#
# Usage:
#   make release.crates.medea-client-api-proto

release.crates.medea-client-api-proto:
	cd proto/client-api && cargo publish --token ${CARGO_TOKEN}


# Build and publish Medea's macro to crates.io
#
# Note that this command will use CARGO_TOKEN enviroment
# variable for publishing.
#
# Usage:
#   make release.crates.medea-macro

release.crates.medea-macro:
	cd crates/medea-macro && cargo publish --token ${CARGO_TOKEN}


release.helm: helm.package.release




###################
# Docker commands #
###################

docker-env = $(strip $(if $(call eq,$(minikube),yes),\
	$(subst export,,$(shell minikube docker-env | cut -d '\#' -f1)),))

# Build Docker image for demo application.
#
# Usage:
#	make docker.build.demo [TAG=(dev|<tag>)]
#	                       [minikube=(no|yes)]

docker-build-demo-image-name = $(DEMO_IMAGE_NAME)

docker.build.demo:
	@make yarn proj=demo
	$(docker-env) \
	docker build $(if $(call eq,$(minikube),yes),,--network=host) --force-rm \
		-t $(docker-build-demo-image-name):$(if $(call eq,$(TAG),),dev,$(TAG)) \
		jason/demo


# Build medea project Docker image.
#
# Usage:
#	make docker.build.medea [TAG=(dev|<tag>)] [debug=(yes|no)]
#	                        [no-cache=(no|yes)]
#	                        [minikube=(no|yes)]

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
	$(docker-env) \
	docker build $(if $(call eq,$(minikube),yes),,--network=host) --force-rm \
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


# Stop demo application in Docker Compose environment
# and remove all related containers.
#
# Usage:
#	make docker.down.demo

docker.down.demo:
	docker-compose -f jason/demo/docker-compose.yml down --rmi=local -v


# Run demo application in Docker Compose environment.
#
# Usage:
#	make docker.up.demo

docker.up.demo: docker.down.demo
	docker-compose -f jason/demo/docker-compose.yml up




##############################
# Helm and Minikube commands #
##############################

helm-cluster = $(if $(call eq,$(cluster),),minikube,$(cluster))
helm-namespace = $(if $(call eq,$(helm-cluster),minikube),kube,staging)-system
helm-cluster-args = $(strip \
	--kube-context=$(helm-cluster) --tiller-namespace=$(helm-namespace))

helm-chart = $(if $(call eq,$(chart),),medea-demo,$(chart))
helm-chart-dir = jason/demo/chart/medea-demo
helm-chart-vals-dir = jason/demo

helm-release = $(if $(call eq,$(release),),,$(release)-)$(helm-chart)
helm-release-namespace = $(strip \
	$(if $(call eq,$(helm-cluster),staging),staging,default))

# Run Helm command in context of concrete Kubernetes cluster.
#
# Usage:
#	make helm [cmd=(--help|'<command>')]
#	          [cluster=(minikube|staging)]

helm:
	helm $(helm-cluster-args) $(if $(call eq,$(cmd),),--help,$(cmd))


# Remove Helm release of project Helm chart from Kubernetes cluster.
#
# Usage:
#	make helm.down [chart=medea-demo] [release=<release-name>]
#	               [cluster=(minikube|staging)]

helm.down:
	$(if $(shell helm ls $(helm-cluster-args) | grep '$(helm-release)'),\
		helm del --purge $(helm-cluster-args) $(helm-release) ,\
		@echo "--> No '$(helm-release)' release found in $(helm-cluster) cluster")


# Upgrade (or initialize) Tiller (server side of Helm) of Minikube.
#
# Usage:
#	make helm.init [client-only=no [upgrade=(yes|no)]]
#	               [client-only=yes]

helm.init:
	helm init --wait \
		$(if $(call eq,$(client-only),yes),\
			--client-only,\
			--kube-context=minikube --tiller-namespace=kube-system \
				$(if $(call eq,$(upgrade),no),,--upgrade))


# Lint project Helm chart.
#
# Usage:
#	make helm.lint [chart=medea-demo]

helm.lint:
	helm lint $(helm-chart-dir)/


# List all Helm releases in Kubernetes cluster.
#
# Usage:
#	make helm.list [cluster=(minikube|staging)]

helm.list:
	helm ls $(helm-cluster-args)


# Build Helm package from project Helm chart.
#
# Usage:
#	make helm.package [chart=medea-demo]

helm-package-dir = .cache/helm/packages

helm.package:
	@rm -rf $(helm-package-dir)
	@mkdir -p $(helm-package-dir)/
	helm package --destination=$(helm-package-dir)/ $(helm-chart-dir)/


# Build and publish project Helm package to GitHub Pages.
#
# Usage:
#	make helm.package.release [chart=medea-demo] [build=(yes|no)]

helm.package.release:
ifneq ($(build),no)
	@make helm.package chart=$(helm-chart)
endif
	git fetch origin gh-pages:gh-pages
	git checkout gh-pages
	git reset --hard
	@mkdir -p charts/
	cp -rf $(helm-package-dir)/* charts/
	if [ -n "$$(git add -v charts/)" ]; then \
		helm repo index charts/ \
			--url=https://instrumentisto.github.io/medea/charts ; \
		git add -v charts/ ; \
		git commit -m "Release '$(helm-chart)' Helm chart" ; \
	fi
	git checkout $(CURRENT_BRANCH)
	git push origin gh-pages


# Run project Helm chart in Kubernetes cluster as Helm release.
#
# Usage:
#	make helm.up [chart=medea-demo] [release=<release-name>]
#	             [cluster=minikube [rebuild=(no|yes) [no-cache=(no|yes)]]]
#	             [cluster=staging]
#	             [wait=(yes|no)]

helm.up:
ifeq ($(wildcard $(helm-chart-vals-dir)/my.$(helm-cluster).vals.yaml),)
	touch $(helm-chart-vals-dir)/my.$(helm-cluster).vals.yaml
endif
ifeq ($(helm-cluster),minikube)
ifeq ($(helm-chart),medea-demo)
ifeq ($(rebuild),yes)
	@make docker.build.demo minikube=yes TAG=dev
	@make docker.build.medea no-cache=$(no-cache) minikube=yes TAG=dev
endif
endif
endif
	helm upgrade --install --force $(helm-cluster-args) \
		$(helm-release) $(helm-chart-dir)/ \
			--namespace=$(helm-release-namespace) \
			--values=$(helm-chart-vals-dir)/$(helm-cluster).vals.yaml \
			--values=$(helm-chart-vals-dir)/my.$(helm-cluster).vals.yaml \
			--set server.deployment.revision=$(shell date +%s) \
			--set web-client.deployment.revision=$(shell date +%s) \
			$(if $(call eq,$(wait),no),,\
				--wait )


# Bootstrap Minikube cluster (local Kubernetes) for development environment.
#
# The bootsrap script is updated automatically to the latest version every day.
# For manual update use 'update=yes' command option.
#
# Usage:
#	make minikube.boot [update=(no|yes)]
#	                   [driver=(virtualbox|hyperkit|hyperv)]
#	                   [k8s-version=<kubernetes-version>]

minikube.boot:
ifeq ($(update),yes)
	$(call minikube.boot.download)
else
ifeq ($(wildcard $(HOME)/.minikube/bootstrap.sh),)
	$(call minikube.boot.download)
else
ifneq ($(shell find $(HOME)/.minikube/bootstrap.sh -mmin +1440),)
	$(call minikube.boot.download)
endif
endif
endif
	@$(if $(cal eq,$(driver),),,MINIKUBE_VM_DRIVER=$(driver)) \
	 $(if $(cal eq,$(k8s-version),),,MINIKUBE_K8S_VER=$(k8s-version)) \
		$(HOME)/.minikube/bootstrap.sh
define minikube.boot.download
	$()
	@mkdir -p $(HOME)/.minikube/
	@rm -f $(HOME)/.minikube/bootstrap.sh
	curl -fL -o $(HOME)/.minikube/bootstrap.sh \
		https://raw.githubusercontent.com/instrumentisto/toolchain/master/minikube/bootstrap.sh
	@chmod +x $(HOME)/.minikube/bootstrap.sh
endef




##################
# .PHONY section #
##################

.PHONY: build \
        cargo cargo.fmt cargo.lint \
        docker.build.demo docker.build.medea docker.down.demo docker.up.demo \
        docs docs.rust \
        down.demo \
        helm helm.down helm.init helm.lint helm.list \
        	helm.package helm.package.release helm.up \
        minikube.boot \
        down down.medea down.coturn \
        release.jason release.crates.jason release.npm.jason release.helm \
        release.crates.medea release.crates.medea-client-api-proto \
        release.crates.medea-macro \ release.helm \
        test test.unit test.e2e \
        up up.coturn up.demo up.dev up.jason up.medea \
        yarn
