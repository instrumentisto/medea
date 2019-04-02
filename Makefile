######################
# Project parameters #
######################

CARGO_HOME ?= $(strip $(shell dirname $$(dirname $$(which cargo))))
RUST_VER ?= "1.33"




###########
# Aliases #
###########

lint: cargo.lint


fmt: cargo.fmt


# Run all project tests.
#
# Usage:
#	make test

test: test.unit





# Lint Rust sources with clippy.
#
# Usage:
#	make cargo.lint [dockerized=(no|yes)]

cargo.lint:
ifeq ($(dockerized),yes)
	docker run --rm --network=host -v "$(PWD)":/app -w /app \
	           -v "$(abspath $(CARGO_HOME))/registry":/usr/local/cargo/registry\
		rust:$(RUST_VER) \
			make cargo.lint dockerized=no pre-install=yes
else
ifeq ($(pre-install),yes)
	rustup component add clippy
endif
	cargo clippy -- -D clippy::pedantic -D warnings
endif



# Generate project documentation of Rust sources.
#
# Usage:
#	make docs [open=(yes|no)] [clean=(no|yes)]

docs:
ifeq ($(clean),yes)
	@rm -rf target/doc/
endif
	cargo +nightly doc $(if $(call eq,$(open),no),,--open)




# Run Rust unit tests of project.
#
# Usage:
#	make test.unit [dockerized=(no|yes)]

test.unit:
ifeq ($(dockerized),yes)
	docker run --rm --network=host -v "$(PWD)":/app -w /app \
	           -v "$(abspath $(CARGO_HOME))/registry":/usr/local/cargo/registry\
		rust:$(RUST_VER) \
			make test.unit dockerized=no
else
	cargo test --all
endif




# Format Rust sources with rustfmt.
#
# Usage:
#	make cargo.fmt [check=(no|yes)]
#	               [dockerized=(no|yes)]

cargo.fmt:
ifeq ($(dockerized),yes)
	docker pull rustlang/rust:nightly
	docker run --rm --network=host -v "$(PWD)":/app -w /app \
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
        docs test test.e2e test.unit
