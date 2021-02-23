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

IMAGE_REPO := instrumentisto
IMAGE_NAME := $(strip \
	$(if $(call eq,$(image),),medea,\
	$(if $(call eq,$(image),medea-demo-edge),medea-demo,\
	$(image))))

RUST_VER := 1.50
CHROME_VERSION := 88.0
FIREFOX_VERSION := 85.0.2

crate-dir = .
ifeq ($(crate),medea-jason)
crate-dir = jason
endif
ifeq ($(crate),medea-client-api-proto)
crate-dir = proto/client-api
endif
ifeq ($(crate),medea-control-api-proto)
crate-dir = proto/control-api
endif
ifeq ($(crate),medea-control-api-mock)
crate-dir = mock/control-api
endif
ifeq ($(crate),medea-macro)
crate-dir = crates/medea-macro
endif
ifeq ($(crate),medea-reactive)
crate-dir = crates/medea-reactive
endif
ifeq ($(crate),medea-coturn-telnet-client)
crate-dir = crates/medea-coturn-telnet-client
endif
crate-ver := $(strip \
	$(shell grep -m1 'version = "' $(crate-dir)/Cargo.toml | cut -d '"' -f2))




###########
# Aliases #
###########

# Build all project executables.
#
# Usage:
#	make build

build: build.medea build.jason


build.medea:
	@make cargo.build crate=medea debug=$(debug) dockerized=$(dockerized)


build.jason:
	@make cargo.build crate=medea-jason debug=$(debug) dockerized=$(dockerized)


# Resolve all project dependencies.
#
# Usage:
#	make deps

deps: cargo yarn


docs: docs.rust


down: down.dev


fmt: cargo.fmt


lint: cargo.lint


# Build and publish project crate everywhere.
#
# Usage:
#	make release crate=(medea|medea-jason|<crate-name>)
#	             [publish=(no|yes)]

release: release.crates release.npm


# Run all project tests.
#
# Usage:
#	make test

test:
	@make test.unit
	@make test.integration up=yes dockerized=no
	@make test.e2e up=yes dockerized=no


up: up.dev




####################
# Running commands #
####################

# Stop non-dockerized Control API mock server.
#
# Usage:
#   make down.control

down.control:
	-kill $(shell pidof medea-control-api-mock)


down.coturn: docker.down.coturn


down.demo: docker.down.demo


# Stop all processes in Medea and Jason development environment.
#
# Usage:
#	make down.dev

down.dev:
	@make docker.down.medea dockerized=no
	@make docker.down.medea dockerized=yes
	@make down.control
	@make docker.down.coturn


down.medea: docker.down.medea


# Run Control API mock server.
#
# Usage:
#  make up.control

up.control:
	make wait.port port=6565
	cargo run -p medea-control-api-mock


up.coturn: docker.up.coturn


up.demo: docker.up.demo


# Run Medea and Jason development environment.
#
# Usage:
#	make up.dev

up.dev: up.coturn
	$(MAKE) -j3 up.jason docker.up.medea up.control


up.medea: docker.up.medea


# Run Jason E2E demo in development mode.
#
# Usage:
#	make up.jason

up.jason:
	npm run start --prefix=jason/e2e-demo




##################
# Cargo commands #
##################

# Resolve Cargo project dependencies.
#
# Usage:
#	make cargo [cmd=(fetch|<cargo-cmd>)]

cargo:
	cargo $(if $(call eq,$(cmd),),fetch,$(cmd))


# Build medea's related crates.
#
# Usage:
#	make cargo.build [crate=(@all|medea|medea-jason)]
#	                 [debug=(yes|no)]
#	                 [dockerized=(no|yes)]

cargo-build-crate = $(if $(call eq,$(crate),),@all,$(crate))

cargo.build:
ifeq ($(cargo-build-crate),@all)
	@make build crate=medea
	@make build crate=medea-jason
endif
ifeq ($(cargo-build-crate),medea)
ifeq ($(dockerized),yes)
	docker run --rm -v "$(PWD)":/app -w /app \
		-u $(shell id -u):$(shell id -g) \
		-v "$(HOME)/.cargo/registry":/usr/local/cargo/registry \
		ghcr.io/instrumentisto/rust:$(RUST_VER) \
			make cargo.build crate=$(cargo-build-crate) \
			                 debug=$(debug) dockerized=no
else
	cargo build --bin medea $(if $(call eq,$(debug),no),--release,)
endif
endif
ifeq ($(cargo-build-crate),medea-jason)
ifeq ($(dockerized),yes)
	docker run --rm --network=host -v "$(PWD)":/app -w /app \
		-u $(shell id -u):$(shell id -g) \
		-v "$(HOME)/.cargo/registry":/usr/local/cargo/registry \
		-v "$(HOME):$(HOME)" \
		-e XDG_CACHE_HOME=$(HOME) \
		ghcr.io/instrumentisto/rust:$(RUST_VER) \
			make cargo.build crate=$(cargo-build-crate) \
			                 debug=$(debug) dockerized=no \
			                 pre-install=yes
else
ifeq ($(pre-install),yes)
	curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
endif
	@rm -rf $(crate-dir)/pkg/
	wasm-pack build -t web $(crate-dir) $(if $(call eq,$(debug),no),,--dev)
endif
endif


# Show permalink to CHANGELOG of a concrete version of project's Cargo crate.
#
# Usage:
#	make cargo.changelog.link [crate=(medea|medea-jason|<crate-name>)]
#	                          [ver=($(crate-ver)|<version>)]

cargo-changelog-link-crate = $(if $(call eq,$(crate),),medea,$(crate))
cargo-changelog-link-ver = $(if $(call eq,$(ver),),$(crate-ver),$(ver))

cargo.changelog.link:
	@printf "https://github.com/instrumentisto/medea/blob/$(cargo-changelog-link-crate)-$(cargo-changelog-link-ver)/$(if $(call eq,$(crate-dir),.),,$(crate-dir)/)CHANGELOG.md#$(shell sed -n '/^## \[$(cargo-changelog-link-ver)\]/{s/^## \[\(.*\)\][^0-9]*\([0-9].*\)/\1--\2/;s/[^0-9a-z-]*//g;p;}' $(crate-dir)/CHANGELOG.md)"


# Format Rust sources with rustfmt.
#
# Usage:
#	make cargo.fmt [check=(no|yes)]

cargo.fmt:
	cargo +nightly fmt --all $(if $(call eq,$(check),yes),-- --check,)


# Generate Rust sources with Cargo's build.rs script.
#
# Usage:
#	make cargo.gen crate=medea-control-api-proto

cargo.gen:
ifeq ($(crate),medea-control-api-proto)
	@rm -rf $(crate-dir)/src/grpc/api*.rs
	cd $(crate-dir)/ && \
	cargo build
endif


# Lint Rust sources with Clippy.
#
# Usage:
#	make cargo.lint

cargo.lint:
	cargo clippy --all -- -D clippy::pedantic -D warnings


# Show version of project's Cargo crate.
#
# Usage:
#	make cargo.version [crate=(medea|medea-jason|<crate-name>)]

cargo.version:
	@printf "$(crate-ver)"




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
#	          [pkg=(e2e|medea-demo)]
#	          [dockerized=(yes|no)]

yarn-cmd = $(if $(call eq,$(cmd),),install --pure-lockfile,$(cmd))
yarn-pkg-dir = $(if $(call eq,$(pkg),medea-demo),jason/demo,jason/e2e-demo)

yarn:
ifneq ($(dockerized),no)
	docker run --rm --network=host -v "$(PWD)":/app -w /app \
	           -u $(shell id -u):$(shell id -g) \
		node:latest \
			make yarn cmd='$(yarn-cmd)' pkg=$(pkg) dockerized=no
else
	yarn --cwd=$(yarn-pkg-dir) $(yarn-cmd)
endif


# Show version of project's Yarn package.
#
# Usage:
#	make cargo.version [pkg=medea-demo]

yarn.version:
	@printf "$(strip $(shell grep -m1 '"version": "' jason/demo/package.json \
	                         | cut -d '"' -f4))"




##########################
# Documentation commands #
##########################

# Generate project documentation of Rust sources.
#
# Usage:
#	make docs.rust [crate=(@all|medea|medea-jason|<crate-name>)]
#	               [open=(yes|no)] [clean=(no|yes)]
#	               [dev=(no|yes)]

docs-rust-crate = $(if $(call eq,$(crate),),@all,$(crate))

docs.rust:
ifeq ($(clean),yes)
	@rm -rf target/doc/
endif
	$(if $(call eq,$(docs-rust-crate),@all),\
		cargo doc --all,\
		cd $(crate-dir)/ && cargo doc)\
			--no-deps \
			$(if $(call eq,$(dev),yes),--document-private-items,) \
			$(if $(call eq,$(open),no),,--open)




####################
# Testing commands #
####################

# Run Rust unit tests of project.
#
# Usage:
#	make test.unit [( [crate=@all]
#	                | crate=(medea|<crate-name>)
#	                | crate=medea-jason
#	                  [browser=(chrome|firefox|default)]
#	                  [timeout=(60|<seconds>)] )]

test-unit-crate = $(if $(call eq,$(crate),),@all,$(crate))
wasm-bindgen-timeout = $(if $(call eq,$(timeout),),60,$(timeout))
webdriver-env = $(if $(call eq,$(browser),firefox),GECKO,CHROME)DRIVER_REMOTE

test.unit:
ifeq ($(test-unit-crate),@all)
	@make test.unit crate=medea-macro
	@make test.unit crate=medea-reactive
	@make test.unit crate=medea-coturn-telnet-client
	@make test.unit crate=medea-client-api-proto
	@make test.unit crate=medea-control-api-proto
	@make test.unit crate=medea-jason
	@make test.unit crate=medea
else
ifeq ($(test-unit-crate),medea)
	cargo test --lib --bin medea
else
ifeq ($(crate),medea-jason)
ifeq ($(browser),default)
	cd $(crate-dir)/ && \
	WASM_BINDGEN_TEST_TIMEOUT=$(wasm-bindgen-timeout) \
	cargo test --target wasm32-unknown-unknown --features mockable
else
	@make docker.up.webdriver browser=$(browser)
	sleep 10
	cd $(crate-dir)/ && \
	$(webdriver-env)="http://127.0.0.1:4444" \
	WASM_BINDGEN_TEST_TIMEOUT=$(wasm-bindgen-timeout) \
	cargo test --target wasm32-unknown-unknown --features mockable
	@make docker.down.webdriver browser=$(browser)
endif
else
	cd $(crate-dir)/ && \
	cargo test --all-features
endif
endif
endif


# Run Rust integration tests of project.
#
# Usage:
#	make test.integration [( [up=no]
#	     				   | up=yes [( [dockerized=no] [debug=(yes|no)]
#			                         | dockerized=yes [tag=(dev|<docker-tag>)]
#			                                          [log=(no|yes)]
#		                                              [log-to-file=(no|yes)] )]
#			                        [wait=(5|<seconds>)] )]

test-integration-env = RUST_BACKTRACE=1 \
	$(if $(call eq,$(log),yes),,RUST_LOG=warn) \
	MEDEA_CONTROL__STATIC_SPECS_DIR=tests/specs/ \
	MEDEA_CONF=tests/medea.config.toml

test.integration:
ifeq ($(up),yes)
ifeq ($(dockerized),yes)
	env $(test-integration-env) \
	make docker.up.medea background=yes
else
	@make up.coturn
	env $(test-integration-env) \
	make up.medea background=yes
endif
	sleep $(if $(call eq,$(wait),),5,$(wait))
endif
ifeq ($(dockerized),yes)
	docker run --rm --network=host -v "$(PWD)":/app -w /app \
	           -v "$(abspath $(CARGO_HOME))/registry":/usr/local/cargo/registry\
		ghcr.io/instrumentisto/rust:$(RUST_VER) \
			make test.integration up=no dockerized=no
else
	RUST_BACKTRACE=1 cargo test --test integration
endif
ifeq ($(up),yes)
	-make down
endif


# Run E2E tests of project.
#
# Usage:
#	make test.e2e [( [up=no]
#	               | up=yes [( [dockerized=no] [debug=(yes|no)]
#	                         | dockerized=yes [tag=(dev|<docker-tag>)]
#	                                          [log=(no|yes)]
#	                                          [log-to-file=(no|yes)] )] )]
#	              [wait=(5|<seconds>)]
#	              [browser=(chrome|firefox)]
#		          [tag=(dev|<tag>)]

test-e2e-env = RUST_BACKTRACE=1 \
	$(if $(call eq,$(log),yes),,RUST_LOG=warn) \
	MEDEA_CONTROL__STATIC_SPECS_DIR=tests/specs/ \
	MEDEA_CONF=tests/medea.config.toml \
	COMPOSE_IMAGE_VER=$(if $(call eq,$(tag),),dev,$(tag))

test.e2e:
ifeq ($(up-test),no)
else
	@make build.jason
ifeq ($(dockerized),yes)
else
	docker run --rm -d --network=host --name e2e-files \
		-v $(PWD)/tests/e2e/index.html:/usr/share/nginx/html/index.html \
		-v $(PWD)/jason/pkg:/usr/share/nginx/html/pkg \
		-v $(PWD)/tests/e2e/nginx.conf:/etc/nginx/nginx.conf \
		nginx:1.19.7-alpine
endif
endif
ifeq ($(up),yes)
ifeq ($(dockerized),yes)
	env $(test-e2e-env) docker-compose -f 'docker-compose.e2e.yml' up -d
	docker-compose -f 'docker-compose.e2e.yml' logs &
	make wait.port port=8080
	make wait.port port=30000
	make wait.port port=8000
else
	@make docker.up.coturn background=yes
	env $(test-e2e-env) \
	make up.medea background=yes
	cargo build -p medea-control-api-mock
	cargo run -p medea-control-api-mock &
	make wait.port port=8000
endif
endif
ifeq ($(up-test),no)
else
	@make docker.up.webdriver
	make wait.port port=4444
endif
ifeq ($(dockerized),yes)
	docker run --rm --network=host -v "$(PWD)":/app -w /app \
	           -v "$(abspath $(CARGO_HOME))/registry":/usr/local/cargo/registry\
		ghcr.io/instrumentisto/rust:$(RUST_VER) \
			make test.e2e up=no dockerized=no up-test=no
else
	cargo test --test e2e
endif
ifeq ($(up),yes)
	-make docker.down.webdriver browser=$(browser)
	-docker-compose -f 'docker-compose.e2e.yml' down
	-docker rm -f e2e-files
	-make down
endif



####################
# Waiting commands #
####################

# Waits for some port on localhost to become open.
#
# Usage:
#   make wait.port [port=<port>]

wait.port:
	while ! timeout 1 bash -c "echo > /dev/tcp/localhost/$(port)"; \
		do sleep 1; done




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
ifneq ($(filter $(crate),medea medea-jason medea-client-api-proto medea-control-api-proto medea-coturn-telnet-client medea-macro medea-reactive),)
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
	@make cargo.build crate=$(crate) debug=no dockerized=no
ifeq ($(publish),yes)
	wasm-pack publish $(crate-dir)/
endif
endif




###################
# Docker commands #
###################

docker-env = $(strip $(if $(call eq,$(minikube),yes),\
	$(subst export,,$(shell minikube docker-env | cut -d '\#' -f1)),))

# Build project Docker image with a given tag.
#
# Usage:
#	make docker.build [debug=(yes|no)] [no-cache=(no|yes)]
#		[image=(medea|medea-control-api-mock|medea-demo|medea-demo-edge)]
#		[tag=(dev|<tag>)]
#		[minikube=(no|yes)]

docker-build-tag = $(if $(call eq,$(tag),),dev,$(tag))
docker-build-dir = .
ifeq ($(image),medea-demo)
docker-build-dir = jason/demo
endif
docker-build-file = $(docker-build-dir)/Dockerfile
ifeq ($(image),medea-control-api-mock)
docker-build-file = mock/control-api/Dockerfile
endif
ifeq ($(image),medea-demo-edge)
docker-build-file = jason/Dockerfile
endif

docker.build:
	$(docker-env) \
	docker build $(if $(call eq,$(minikube),yes),,--network=host) --force-rm \
		$(if $(call eq,$(no-cache),yes),\
			--no-cache --pull,) \
		--build-arg rust_ver=$(RUST_VER) \
		--build-arg rustc_mode=$(if $(call eq,$(debug),no),release,debug) \
		--build-arg rustc_opts=$(if $(call eq,$(debug),no),--release,) \
		--build-arg debug=$(if $(call eq,$(debug),no),no,yes) \
		-t $(IMAGE_REPO)/$(IMAGE_NAME):$(docker-build-tag) \
		-f $(docker-build-file) $(docker-build-dir)/


# Stop dockerized Control API mock server and remove all related containers.
#
# Usage:
#   make docker.down.control

docker.down.control:
	-docker stop medea-control-api-mock


# Stop Coturn STUN/TURN server in Docker Compose environment
# and remove all related containers.
#
# Usage:
# 	make docker.down.coturn

docker.down.coturn:
	docker-compose -f docker-compose.coturn.yml down --rmi=local -v


# Stop demo application in Docker Compose environment
# and remove all related containers.
#
# Usage:
#	make docker.down.demo

docker.down.demo:
	docker-compose -f jason/demo/docker-compose.yml down --rmi=local -v


# Stop Medea media server in Docker Compose environment
# and remove all related containers.
#
# Usage:
# 	make docker.down.medea [dockerized=(no|yes)]

docker.down.medea:
ifeq ($(dockerized),yes)
	docker-compose -f docker-compose.medea.yml down --rmi=local -v
else
	-killall medea
endif


# Stop dockerized WebDriver and remove all related containers.
#
# Usage:
#   make docker.down.webdriver [browser=(chrome|firefox)]

docker.down.webdriver:
	-docker stop medea-webdriver-$(if $(call eq,$(browser),),chrome,$(browser))


# Pull project Docker images from Container Registry.
#
# Usage:
#	make docker.pull
#		[image=(medea|medea-control-api-mock|medea-demo)]
#		[repos=($(IMAGE_REPO)|<prefix-1>[,<prefix-2>...])]
#		[tags=(@all|<t1>[,<t2>...])]
#		[minikube=(no|yes)]

docker-pull-repos = $(if $(call eq,$(repos),),$(IMAGE_REPO),$(repos))
docker-pull-tags = $(if $(call eq,$(tags),),@all,$(tags))

docker.pull:
ifeq ($(docker-pull-tags),@all)
	$(foreach repo,$(subst $(comma), ,$(docker-pull-repos)),\
		$(call docker.pull.do,$(repo)/$(IMAGE_NAME) --all-tags))
else
	$(foreach tag,$(subst $(comma), ,$(docker-pull-tags)),\
		$(foreach repo,$(subst $(comma), ,$(docker-pull-repos)),\
			$(call docker.pull.do,$(repo)/$(IMAGE_NAME):$(tag))))
endif
define docker.pull.do
	$(eval image-full := $(strip $(1)))
	$(docker-env) \
	docker pull $(image-full)
endef


# Push project Docker images to Container Registry.
#
# Usage:
#	make docker.push
#		[image=(medea|medea-control-api-mock|medea-demo)]
#		[repos=($(IMAGE_REPO)|<prefix-1>[,<prefix-2>...])]
#		[tags=(dev|<t1>[,<t2>...])]
#		[minikube=(no|yes)]

docker-push-repos = $(if $(call eq,$(repos),),$(IMAGE_REPO),$(repos))
docker-push-tags = $(if $(call eq,$(tags),),dev,$(tags))

docker.push:
	$(foreach tag,$(subst $(comma), ,$(docker-push-tags)),\
		$(foreach repo,$(subst $(comma), ,$(docker-push-repos)),\
			$(call docker.push.do,$(repo)/$(IMAGE_NAME):$(tag))))
define docker.push.do
	$(eval image-full := $(strip $(1)))
	$(docker-env) \
	docker push $(image-full)
endef


# Tag project Docker image with given tags.
#
# Usage:
#	make docker.tag [of=(dev|<tag>)]
#		[image=(medea|medea-control-api-mock|medea-demo)]
#		[repos=($(IMAGE_REPO)|<with-prefix-1>[,<with-prefix-2>...])]
#		[tags=(dev|<with-t1>[,<with-t2>...])]
#		[minikube=(no|yes)]

docker-tag-of := $(if $(call eq,$(of),),dev,$(of))
docker-tag-with := $(if $(call eq,$(tags),),dev,$(tags))
docker-tag-repos = $(if $(call eq,$(repos),),$(IMAGE_REPO),$(repos))

docker.tag:
	$(foreach tag,$(subst $(comma), ,$(docker-tag-with)),\
		$(foreach repo,$(subst $(comma), ,$(docker-tag-repos)),\
			$(call docker.tag.do,$(repo),$(tag))))
define docker.tag.do
	$(eval repo := $(strip $(1)))
	$(eval tag := $(strip $(2)))
	$(docker-env) \
	docker tag $(IMAGE_REPO)/$(IMAGE_NAME):$(if $(call eq,$(of),),dev,$(of)) \
	           $(repo)/$(IMAGE_NAME):$(tag)
endef


# Save project Docker images to a tarball file.
#
# Usage:
#	make docker.tar [to-file=(.cache/image.tar|<file-path>)]
#		[image=(medea|medea-control-api-mock|medea-demo)]
#		[tags=(dev|<t1>[,<t2>...])]
#		[minikube=(no|yes)]

docker-tar-file = $(if $(call eq,$(to-file),),.cache/image.tar,$(to-file))
docker-tar-tags = $(if $(call eq,$(tags),),dev,$(tags))

docker.tar:
	@mkdir -p $(dir $(docker-tar-file))
	$(docker-env) \
	docker save -o $(docker-tar-file) \
		$(foreach tag,$(subst $(comma), ,$(docker-tar-tags)),\
			$(IMAGE_REPO)/$(IMAGE_NAME):$(tag))


# Load project Docker images from a tarball file.
#
# Usage:
#	make docker.untar [from-file=(.cache/image.tar|<file-path>)]
#		[minikube=(no|yes)]

docker-untar-file = $(if $(call eq,$(from-file),),.cache/image.tar,$(from-file))

docker.untar:
	$(docker-env) \
	docker load -i $(docker-untar-file)


# Run dockerized Medea Control API mock server.
#
# Usage:
#   make docker.up.control [tag=(dev|<docker-tag>)]

docker.up.control:
	docker run --rm -d --network=host \
		--name medea-control-api-mock \
		$(IMAGE_REPO)/medea-control-api-mock:$(if $(call eq,$(tag),),dev,$(tag))


# Run Coturn STUN/TURN server in Docker Compose environment.
#
# Usage:
#	make docker.up.coturn [background=(yes|no)]

docker.up.coturn: docker.down.coturn
	docker-compose -f docker-compose.coturn.yml up \
		$(if $(call eq,$(background),no),--abort-on-container-exit,-d)


# Run demo application in Docker Compose environment.
#
# Usage:
#	make docker.up.demo

docker.up.demo: docker.down.demo
	docker-compose -f jason/demo/docker-compose.yml up


# Run Medea media server in Docker Compose environment.
#
# Usage:
#	make docker.up.medea [( [dockerized=no] [debug=(yes|no)]
#	                                        [background=(no|yes)]
#	                      | dockerized=yes [tag=(dev|<docker-tag>)]
#	                                       [( [background=no]
#	                                        | background=yes [log=(no|yes)] )])]
#	                     [log-to-file=(no|yes)]

docker-up-medea-image = $(IMAGE_REPO)/medea
docker-up-medea-tag = $(if $(call eq,$(tag),),dev,$(tag))

docker.up.medea: docker.down.medea
ifeq ($(dockerized),yes)
	COMPOSE_IMAGE_NAME=$(docker-up-medea-image) \
	COMPOSE_IMAGE_VER=$(docker-up-medea-tag) \
	docker-compose -f docker-compose.medea.yml up \
		$(if $(call eq,$(background),yes),-d,--abort-on-container-exit)
ifeq ($(background),yes)
ifeq ($(log),yes)
	docker-compose -f docker-compose.medea.yml logs -f &
endif
endif
else
ifeq ($(log-to-file),yes)
	@rm -f /tmp/medea.log
endif
	cargo build --bin medea $(if $(call eq,$(debug),no),--release,)
	cargo run --bin medea $(if $(call eq,$(debug),no),--release,) \
		$(if $(call eq,$(log-to-file),yes),> /tmp/medea.log,) \
		$(if $(call eq,$(background),yes),&,)
endif


# Run dockerized WebDriver.
#
# Usage:
#   make docker.up.webdriver [browser=(chrome|firefox)]

docker.up.webdriver:
	-@make docker.down.webdriver browser=chrome
	-@make docker.down.webdriver browser=firefox
ifeq ($(browser),firefox)
	docker run --rm -d --network=host --shm-size 512m \
		--name medea-webdriver-firefox \
		ghcr.io/instrumentisto/geckodriver:$(FIREFOX_VERSION)
else
	docker run --rm -d --network=host \
		--name medea-webdriver-chrome \
		selenoid/chrome:$(CHROME_VERSION)
endif




##############################
# Helm and Minikube commands #
##############################

helm-cluster = $(if $(call eq,$(cluster),),minikube,$(cluster))
helm-cluster-args = --kube-context=$(helm-cluster)

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


# Show root directory path of project Helm chart.
#
# Usage:
#	make helm.dir [chart=medea-demo]

helm.dir:
	@printf "$(helm-chart-dir)"


# Remove Helm release of project Helm chart from Kubernetes cluster.
#
# Usage:
#	make helm.down [chart=medea-demo] [release=<release-name>]
#	               [cluster=(minikube|staging)]
#	               [check=(no|yes)]

helm.down:
ifeq ($(check),yes)
	$(if $(shell helm $(helm-cluster-args) list | grep '$(helm-release)'),\
		helm $(helm-cluster-args) uninstall $(helm-release) ,\
		@echo "--> No $(helm-release) release found in $(helm-cluster) cluster")
else
	helm $(helm-cluster-args) uninstall $(helm-release)
endif


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
	helm $(helm-cluster-args) list


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

helm-package-release-ver := $(strip $(shell \
	grep 'version: ' jason/demo/chart/medea-demo/Chart.yaml | cut -d ':' -f2))

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
		git commit -m \
			"Release $(helm-chart)-$(helm-package-release-ver) Helm chart" ; \
	fi
	git checkout -
	git push origin gh-pages


# Run project Helm chart in Kubernetes cluster as Helm release.
#
# Usage:
#	make helm.up [chart=medea-demo] [release=<release-name>]
#	             [force=(no|yes)]
#	             [( [atomic=no] [wait=(yes|no)]
#	              | atomic=yes )]
#	             [( [cluster=minikube] [( [rebuild=no]
#	                                    | rebuild=yes [no-cache=(no|yes)] )]
#	              | cluster=staging )]

helm.up:
ifeq ($(wildcard $(helm-chart-vals-dir)/my.$(helm-cluster).vals.yaml),)
	touch $(helm-chart-vals-dir)/my.$(helm-cluster).vals.yaml
endif
ifeq ($(helm-cluster),minikube)
ifeq ($(helm-chart),medea-demo)
ifeq ($(rebuild),yes)
	@make docker.build image=medea-demo-edge tag=dev \
	                   minikube=yes no-cache=$(no-cache)
	@make docker.build image=medea tag=dev \
	                   minikube=yes no-cache=$(no-cache)
	@make docker.build image=medea-control-api-mock tag=dev \
	                   minikube=yes no-cache=$(no-cache)
endif
endif
endif
	helm $(helm-cluster-args) upgrade --install \
		$(helm-release) $(helm-chart-dir)/ \
			--namespace=$(helm-release-namespace) \
			--values=$(helm-chart-vals-dir)/$(helm-cluster).vals.yaml \
			--values=$(helm-chart-vals-dir)/my.$(helm-cluster).vals.yaml \
			--set server.deployment.revision=$(shell date +%s) \
			--set web-client.deployment.revision=$(shell date +%s) \
			$(if $(call eq,$(force),yes),\
				--force,)\
			$(if $(call eq,$(atomic),yes),\
				--atomic,\
			$(if $(call eq,$(wait),no),,\
				--wait ))


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

.PHONY: build build.jason build.medea \
        cargo cargo.build cargo.changelog.link cargo.fmt cargo.gen cargo.lint \
        	cargo.version \
        docker.build \
        	docker.down.control docker.down.coturn docker.down.demo \
        	docker.down.medea docker.down.webdriver  \
        	docker.pull docker.push docker.tag docker.tar docker.untar \
        	docker.up.control docker.up.coturn docker.up.demo docker.up.medea \
        	docker.up.webdriver \
        docs docs.rust \
        down down.control down.coturn down.demo down.dev down.medea \
        helm helm.dir helm.down helm.lint helm.list \
        	helm.package helm.package.release helm.up \
        minikube.boot \
        release release.crates release.helm release.npm \
        test test.e2e test.integration test.unit \
        up up.control up.coturn up.demo up.dev up.jason up.medea \
        wait.port \
        yarn yarn.version
