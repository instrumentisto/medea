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
