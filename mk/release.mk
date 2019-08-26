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
