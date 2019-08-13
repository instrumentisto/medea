## How to release this repository

All releases in this repository normally are made with [Travis CI] using git tags.
Read [this][2] if you don't understand what is git tag.

__Before publishing every crate (no matters how) run `cargo package` in crate directory.__ 
This will check crate for some errors important for publishing.

### Git tags for releasing
These tags will only be searched on the master branch.

`x.y.z` - is version of crate. Note that this version will not be checked for
sameness with version from `Cargo.toml`. This only affect on git releasing tag
and triggering deploy job on [Travis CI].

- `medea-macro-x.y.z` - publish medea-macro crate to [crates.io]
- `medea-x.y.z` - publish medea to [crates.io]
- `medea-jason-x.y.z` - publish medea-jason to [crates.io] and [NPM]
- `medea-client-api-proto-x.y.z` - publish medea-client-api-proto to [crates.io]
- all tags (including those listed above) - create release on Github with specified tag

### Preparation of crate to release
#### 1. Change declaration of internal dependencies from path to version.
Internal dependencies probably will be declared like this:
`medea-macro = { path = "crates/medea-macro" }`. This declaration is good
for developing but when we release crate all it deps should be published on
[crates.io]. Simply change this declaration to something like that:
`medea-macro = "1.2.3"`. Most likely in place of "1.2.3" there will be the
latest version of the crate that you previously released.

__Be sure to follow the rules in the "Order of releasing" section of this guide.__

#### 2. Remove `-dev` postfix from version of crate
Simply remove `-dev` in `version` field in `Cargo.toml` of crate which
you want release. For example, `1.2.3-dev` to `1.2.3`.

#### 3. Check with `$ cargo package`
Fix all errors (if any will) which this command will output.

#### 4. Set version in `CHANGELOG.md` of crate

### Order of releasing
__The order of releasing of the crates can be important.__ For example, you want to 
release `medea-1.2.3` which uses latest unreleased `medea-macro` (version will be 1.3.6).
In such case [Travis CI] deploy job will fail because cargo can't find 
specified version of `medea-macro`. 

For avoid it use following flow:

1. Prepare and release `medea-macro-1.3.6` (apply `medea-macro-1.3.6` git tag)
2. Change `medea-macro = { path = crates/medea-macro }` to `medea-macro = "1.3.6"`
   in medea's `Cargo.toml` and `medea-client-api-proto`'s `Cargo.toml`.
3. Prepare and release `medea-client-api-proto` (apply `medea-client-api-proto-1.2.4` git tag)
4. Change `medea-client-api-proto = { path = "proto/client-api" }` to 
   `medea-client-api-proto = "1.2.4"` in `medea`'s `Cargo.toml`
5. Prepare and release `medea` (apply `medea-1.2.3` git tag)

If you wish also release `medea-jason` which uses same version of `medea-macro`, then
you can simply do it (prepare `medea-jason` for release and apply 
`medea-jason-x.y.z` tag).

#### Current releasing priority
1. `medea-macro` (used by `medea`, `medea-client-api-proto` and `medea-jason`)
2. `medea-client-api-proto` (used by `medea` and `medea-jason`)
3. `medea` and `medea-jason`

### Releasing with Makefile
__Use it only if publishing with [Travis CI] is not possible for some reason 
([Travis CI] is down for example).__

For publishing to [crates.io] will be used token from `CARGO_TOKEN` environment variable.
And for [NPM] will be used your local token for npm.

`Makefile` contains the following recipes for releasing:

- `$ make release.jason` - publish `medea-jason` to [crates.io] and [NPM]
- `$ make release.crates.jason` - publish `medea-jason` __only__ to [crates.io]
- `$ make release.npm.jason` - publish `medea-jason` __only__ to [NPM]
- `$ make release.crates.medea` - publish `medea` to [crates.io]
- `$ make release.crates.medea-client-api-proto` - publish `medea-client-api-proto`
  to [crates.io]
- `$ make release.crates.medea-macro` - publish `medea-macro` to [crates.io]

For manually create Github release you can follow [this guide][1].

### After release
When you released everything you wanted you should transfer everything to dev state.

1. Set version according milestone and add `-dev` postfix to it
2. Add section for next milestone in `CHANGELOG.md`
3. Change all internal deps declaration from version to path 
   (`medea-macro = "1.0.0"` -> `medea-macro = { path = "crates/medea-macro" }`)
4. Commit it to master

### I broke everything. Help!
#### If broken crate released to [crates.io]
1. Yank broken version of crate on [crates.io] 
   (read [this][3] if you don't know what is it)
2. Fix crate
3. Bump PATCH version of this crate everywhere
4. Publish it

#### If some error occurred when releasing in CI
1. Fix errors from CI and commit
2. Force set tag to commit with fix (`$ git tag -fa {{YOUR_TAG_HERE}}`)
3. Force push this tag (`$ git push -f origin {{YOUR_TAG_HERE}}`)
4. Profit! [Travis CI] will rerun deploy job 


[1]: https://help.github.com/en/articles/creating-releases
[2]: https://git-scm.com/book/en/v2/Git-Basics-Tagging
[3]: https://doc.rust-lang.org/cargo/reference/publishing.html#cargo-yank
[crates.io]: https://crates.io/
[NPM]: https://www.npmjs.com/
[Travis CI]: https://travis-ci.org/
