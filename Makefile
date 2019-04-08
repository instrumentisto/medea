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
RUST_VER ?= 1.33




###########
# Aliases #
###########

docs: docs.rust


lint: cargo.lint


fmt: cargo.fmt


test: test.unit




##################
# Cargo commands #
##################

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
	cargo clippy -- -D clippy::pedantic -D warnings




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
#	make test.unit

test.unit:
	cargo test --all




##################
# .PHONY section #
##################

.PHONY: cargo cargo.fmt cargo.lint \
        docs docs.rust \
        test test.unit
