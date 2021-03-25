eq = $(if $(or $(1),$(2)),$(and $(findstring $(1),$(2)),\
                                $(findstring $(2),$(1))),1)

release_flag = $(if $(call eq,$(release),yes),--release,)

ios: ios-build ios-copy-lib

ios-build:
	@make switch-to-ios-crate-type
	cd rust && cargo lipo $(release_flag)

ios-copy-lib:
	cp rust/target/universal/debug/librust_ffi.a \
	   jason/ios

android-build:
	cd rust && cargo ndk --platform 28 --target ${target} -o ../jason/android/src/main/jniLibs build $(release_flag)

switch-to-android-crate-type:
	perl -pi -e "s/staticlib/cdylib/g" rust/Cargo.toml

switch-to-ios-crate-type:
	perl -pi -e "s/cdylib/staticlib/g" rust/Cargo.toml

android:
	@make switch-to-android-crate-type
ifeq ($(target),)
	@make android-build target=arm64-v8a
	@make android-build target=armeabi-v7a
	@make android-build target=x86
	@make android-build target=x86_64
else
	@make android-build
endif
	@make switch-to-ios-crate-type

.PHONY: ios android android-build ios-copy-lib ios-build