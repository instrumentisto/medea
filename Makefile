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


# Stop all services needed for e2e testing of medea in browsers.
#
# Usage:
#   make down.e2e.services [dockerized=(yes|no)] [coturn=(yes|no)]

down.e2e.services:
ifeq ($(dockerized),no)
	kill $$(cat /tmp/e2e_medea.pid)
	kill $$(cat /tmp/e2e_control_api_mock.pid)
	rm -f /tmp/e2e_medea.pid \
		/tmp/e2e_control_api_mock.pid
ifneq ($(coturn),no)
	@make down.coturn
endif
else
	docker container stop $$(cat /tmp/control-api-mock.docker.uid)
	docker container stop $$(cat /tmp/medea.docker.uid)
	rm -f /tmp/control-api-mock.docker.uid /tmp/medea.docker.uid

	@make down.coturn
endif


# Stop dockerized coturn.
#
# Usage:
# 	make down.coturn

down.coturn:
	docker-compose -f docker-compose.coturn.yml down


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
	mkdir -p .cache target ~/.cargo/registry
	@make down.medea dockerized=yes
	@make down.medea dockerized=no
	@make up.coturn

	# TODO: publish it to docker hub
	@make docker.build.medea-build

	$(run-medea-container) sh -c "cd jason && RUST_LOG=info wasm-pack build --target web --out-dir ../.cache/jason-pkg"

	$(run-medea-container) make build.medea optimized=yes
	$(run-medea-container-d) cargo run > /tmp/medea.docker.uid

	$(run-medea-container) cargo build -p control-api-mock
	$(run-medea-container-d) cargo run -p control-api-mock > /tmp/control-api-mock.docker.uid

	$(run-medea-container) cargo build -p e2e-tests-runner
endif




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
#	make cargo.fmt [check=(no|yes)] [build=(no|yes)]

cargo.fmt:
ifeq ($(build),yes)
	cargo build
endif
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
	mkdir -p .cache target ~/.cargo/registry
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




######################
# Releasing commands #
######################

# Build and publish project crate to crates.io.
#
# Usage:
#	make release.crates crate=(medea|medea-jason|<crate-name>)
#	                    [token=($CARGO_TOKEN|<cargo-token>)]
#	                    [publish=(no|yes)]

release-crates-token = $(if $(call eq,$(token),),${CARGO_TOKEN},$(token))

release.crates:
ifneq ($(filter $(crate),medea medea-jason medea-client-api-proto medea-macro),)
	cd $(crate-dir)/ && \
	$(if $(call eq,$(publish),yes),\
		cargo publish --token $(release-crates-token) ,\
		cargo package --allow-dirty )
endif


release.helm: helm.package.release


# Build and publish project crate to NPM.
#
# Usage:
#	make release.npm crate=medea-jason
#	                 [publish=(no|yes)]

release.npm:
ifneq ($(filter $(crate),medea-jason),)
	@rm -rf $(crate-dir)/pkg/
	wasm-pack build -t web $(crate-dir)/
ifeq ($(publish),yes)
	wasm-pack publish $(crate-dir)/
endif
endif




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


# Build Dockerfile for medea building and tag it with 'medea-build' tag.
#
# Usage:
#   make docker.build.medea-build

docker.build.medea-build:
	docker build -t medea-build -f build/medea/Dockerfile .




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




############
# Building #
############

# Build medea.
#
# Usage:
#   make build.medea [dockerized=(no|yes)] [optimized=(no|yes)]

build.medea:
ifneq ($(dockerized),yes)
	cargo build --bin medea $(if $(call eq,$(optimized),yes),--release)
else
	docker run --rm \
		-v "$(PWD)":/app -w /app \
		-u $(shell id -u):$(shell id -g) \
		-v "$(HOME)/.cargo/registry":/usr/local/cargo/registry \
		-v "$(PWD)/target":/app/target \
		rust:latest \
		make build.medea optimized=$(optimized)
endif


# Build jason.
#
# Usage:
#   make build.jason [dockerized=(no|yes)]

build.jason:
ifneq ($(dockerized),yes)
	wasm-pack build -t web jason
else
	docker run --rm --network=host \
		-v "$(PWD)":/app -w /app \
		-u $(shell id -u):$(shell id -g) \
		-v "$(HOME)/.cargo/registry":/usr/local/cargo/registry \
		-v "$(PWD)/target":/app/target \
		--env XDG_CACHE_HOME=$(HOME) \
		-v "$(HOME):$(HOME)" \
		rust:latest \
		sh -c "curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh && make build.jason"
endif




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
